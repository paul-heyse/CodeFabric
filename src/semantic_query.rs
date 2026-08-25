//! Bounded semantic-query ingress and DataFusion execution over one pinned snapshot.

use std::collections::BTreeMap;

use arrow_array::{Array as _, FixedSizeBinaryArray};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

use crate::contracts::jcs::{CanonicalJsonError, canonicalize_slice, canonicalize_value};
use crate::fabric::{ServingQueryError, ServingQuerySession};
pub use crate::registries::QueryForm;
use crate::registries::{PHRASE_ENTRIES, PhraseEntry, QUERY_FORM_VALUES};

const SPECIFICATION: &str = "composable semantic CPG fact query";
const VERSION: &str = "1.3";
const MAX_REQUEST_BYTES: usize = 256 * 1024;
const MAX_QUERIES: usize = 32;
const MAX_ROWS_PER_QUERY: usize = 10_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessPolicy {
    CurrentRequired,
    WaitForCurrent,
    BestAvailableSnapshot,
}

impl QueryForm {
    pub(crate) const fn currently_supported(self) -> bool {
        matches!(
            self,
            Self::FindEntities | Self::RetrieveFacts | Self::FollowRelationships
        )
    }

    pub(crate) fn registry_slug(self) -> &'static str {
        QUERY_FORM_VALUES
            .iter()
            .find(|entry| entry.code == self as u16)
            .expect("generated QueryForm and QUERY_FORM_VALUES are one authority")
            .slug
    }

    fn table(self) -> Result<&'static str, SemanticQueryError> {
        match self {
            Self::FindEntities => Ok("entities"),
            Self::RetrieveFacts => Ok("properties"),
            Self::FollowRelationships => Ok("relations"),
            _ => Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            )),
        }
    }

    fn order_key(self) -> Result<&'static str, SemanticQueryError> {
        match self {
            Self::FindEntities => Ok("entity_id"),
            Self::RetrieveFacts | Self::FollowRelationships => Ok("fact_id"),
            _ => Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            )),
        }
    }

    fn plan_node_kind(self) -> Result<&'static str, SemanticQueryError> {
        match self {
            Self::FindEntities => Ok("find-entities"),
            Self::RetrieveFacts => Ok("retrieve-facts"),
            Self::FollowRelationships => Ok("follow-relationships"),
            _ => Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            )),
        }
    }
}

impl Serialize for QueryForm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.registry_slug())
    }
}

impl<'de> Deserialize<'de> for QueryForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let slug = String::deserialize(deserializer)?;
        let entry = QUERY_FORM_VALUES
            .iter()
            .find(|entry| entry.slug == slug)
            .ok_or_else(|| D::Error::custom("unknown governed query form"))?;
        Self::try_from(entry.code).map_err(|_| D::Error::custom("invalid governed query form"))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryInput {
    #[serde(default)]
    pub entity_ids: Vec<String>,
    #[serde(default)]
    pub fact_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryPredicate {
    #[serde(default)]
    pub entity_kind_codes: Vec<u32>,
    #[serde(default)]
    pub relation_kind_codes: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryLimit {
    pub first: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticQueryClause {
    pub query_id: String,
    pub request: QueryForm,
    pub label: Option<String>,
    pub input: Option<QueryInput>,
    pub r#where: Option<QueryPredicate>,
    pub limit: Option<QueryLimit>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CostBudget {
    pub maximum_rows: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticQueryRequest {
    pub specification: String,
    pub version: String,
    pub semantic_request_id: String,
    pub workspace_id: String,
    pub freshness_policy: FreshnessPolicy,
    pub queries: Vec<SemanticQueryClause>,
    pub response_projection: Option<BTreeMap<String, bool>>,
    pub cost_budget: Option<CostBudget>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryResultRecord {
    pub query_id: String,
    pub request: QueryForm,
    pub execution_state: &'static str,
    pub availability_state: &'static str,
    pub completeness_state: &'static str,
    pub freshness_state: &'static str,
    pub limit_state: &'static str,
    pub dependency_state: &'static str,
    pub resolved_semantics: BTreeMap<String, String>,
    pub entity_ids: Vec<String>,
    pub fact_ids: Vec<String>,
    pub path_ids: Vec<String>,
    pub group_ids: Vec<String>,
    pub source_context_ids: Vec<String>,
    pub coverage: BTreeMap<String, u64>,
    pub errors: Vec<SemanticErrorRecord>,
    pub notices: Vec<String>,
    #[serde(skip)]
    pub output_row_count: usize,
    #[serde(skip)]
    pub result_checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticErrorRecord {
    pub code: String,
    pub layer: String,
    pub retryable: bool,
    pub safe_message: String,
    pub field: Option<String>,
    pub semantic_phrase: Option<String>,
    pub candidate_interpretations: Vec<String>,
    pub failed_dependency_query_id: Option<String>,
    pub diagnostic_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticSnapshotResponse {
    pub snapshot_id: String,
    pub workspace_id: String,
    pub repository_id: Option<String>,
    pub worktree_id: Option<String>,
    pub source_generation: u64,
    pub source_inventory_digest: String,
    pub durable_base_publication: String,
    pub base_table_version_digest: String,
    pub overlay_generation: u64,
    pub overlay_checksum: String,
    pub analysis_context_set_id: String,
    pub analysis_context_ids: Vec<String>,
    pub freshness_state: &'static str,
    pub source_trust_state: String,
    pub event_stream_health: String,
    pub git_acceleration_status: String,
    pub git_operation_summary: Option<BTreeMap<String, String>>,
    pub pending_update_count: u64,
    pub ontology_version: String,
    pub schema_bundle_version: String,
    pub provider_bundle_version: String,
    pub derivation_bundle_version: String,
    pub query_language_version: String,
    pub capability_summaries: Vec<BTreeMap<String, String>>,
    pub diagnostic_references: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticQueryResponse {
    pub specification: &'static str,
    pub version: &'static str,
    pub semantic_request_id: String,
    pub execution_state: &'static str,
    pub availability_state: &'static str,
    pub completeness_state: &'static str,
    pub freshness_state: &'static str,
    pub limit_state: &'static str,
    pub successful_query_count: usize,
    pub failed_query_count: usize,
    pub not_executed_dependency_count: usize,
    pub snapshot: SemanticSnapshotResponse,
    pub entities: BTreeMap<String, BTreeMap<String, String>>,
    pub facts: BTreeMap<String, BTreeMap<String, String>>,
    pub paths: BTreeMap<String, BTreeMap<String, String>>,
    pub groups: BTreeMap<String, BTreeMap<String, String>>,
    pub source_contexts: BTreeMap<String, BTreeMap<String, String>>,
    pub query_results: Vec<QueryResultRecord>,
    pub errors: Vec<SemanticErrorRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSemanticRequest {
    pub request: SemanticQueryRequest,
    pub canonical_bytes: Vec<u8>,
    pub request_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutedSemanticResponse {
    pub response: SemanticQueryResponse,
    pub canonical_bytes: Vec<u8>,
    pub response_digest: String,
}

#[derive(Debug, Error)]
pub enum SemanticQueryError {
    #[error("SEMANTIC_QUERY_INVALID:{0}")]
    Invalid(String),
    #[error(transparent)]
    Canonical(#[from] CanonicalJsonError),
    #[error(transparent)]
    Serving(#[from] ServingQueryError),
}

fn b3(bytes: &[u8]) -> String {
    crate::integrity::framed_digest(bytes)
}

fn valid_id(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

fn resolve_phrase(
    form: QueryForm,
    label: Option<&str>,
) -> Result<Option<&'static PhraseEntry>, SemanticQueryError> {
    let Some(label) = label else {
        return Ok(None);
    };
    if label.is_empty() || label.len() > 256 {
        return Err(SemanticQueryError::Invalid(
            "semantic phrase is empty or exceeds its bound".to_owned(),
        ));
    }
    let plan_node_kind = form.plan_node_kind()?;
    let mut matches = PHRASE_ENTRIES.iter().filter(|entry| {
        (entry.canonical_text == label || entry.accepted_aliases.contains(&label))
            && entry.plan_node_kind == plan_node_kind
    });
    let resolved = matches.next().ok_or_else(|| {
        SemanticQueryError::Invalid(
            "semantic phrase is unknown or incompatible with the request form".to_owned(),
        )
    })?;
    if matches.next().is_some() {
        return Err(SemanticQueryError::Invalid(
            "semantic phrase resolves ambiguously".to_owned(),
        ));
    }
    Ok(Some(resolved))
}

/// Strictly decode, normalize, and budget one semantic query request.
///
/// # Errors
///
/// Returns an error for non-canonical or invalid JSON, unknown semantics, incompatible request
/// forms, invalid identifiers, or requests that exceed the registered bounds.
pub fn validate_request(bytes: &[u8]) -> Result<ValidatedSemanticRequest, SemanticQueryError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(SemanticQueryError::Invalid(
            "request exceeds maximum bytes".to_owned(),
        ));
    }
    let canonical_bytes = canonicalize_slice(bytes)?;
    let request: SemanticQueryRequest = serde_json::from_slice(&canonical_bytes)
        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?;
    if request.specification != SPECIFICATION
        || request.version != VERSION
        || !valid_id(&request.semantic_request_id, 128)
        || !valid_id(&request.workspace_id, 128)
        || request.queries.is_empty()
        || request.queries.len() > MAX_QUERIES
    {
        return Err(SemanticQueryError::Invalid(
            "request identity, version, or query count is invalid".to_owned(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    let total_rows = request.queries.iter().try_fold(0_usize, |total, query| {
        if !valid_id(&query.query_id, 128) || !ids.insert(query.query_id.as_str()) {
            return Err(SemanticQueryError::Invalid(
                "query IDs must be unique bounded identifiers".to_owned(),
            ));
        }
        if !query.request.currently_supported() {
            return Err(SemanticQueryError::Invalid(
                "query form is registered but not active in the current execution profile"
                    .to_owned(),
            ));
        }
        resolve_phrase(query.request, query.label.as_deref())?;
        if query
            .input
            .as_ref()
            .is_some_and(|input| !input.entity_ids.is_empty() || !input.fact_ids.is_empty())
            || query.r#where.as_ref().is_some_and(|predicate| {
                !predicate.entity_kind_codes.is_empty() || !predicate.relation_kind_codes.is_empty()
            })
        {
            return Err(SemanticQueryError::Invalid(
                "Wave-5 query filters are outside the accepted minimal subset".to_owned(),
            ));
        }
        let limit = query.limit.unwrap_or(QueryLimit {
            first: 100,
            offset: 0,
        });
        if limit.first == 0 || limit.first > MAX_ROWS_PER_QUERY || limit.offset > 1_000_000 {
            return Err(SemanticQueryError::Invalid(
                "query pagination is outside the accepted bound".to_owned(),
            ));
        }
        total
            .checked_add(limit.first)
            .ok_or_else(|| SemanticQueryError::Invalid("request row budget overflow".to_owned()))
    })?;
    if request
        .cost_budget
        .is_some_and(|budget| budget.maximum_rows == 0 || total_rows > budget.maximum_rows)
    {
        return Err(SemanticQueryError::Invalid(
            "query limits exceed the request cost budget".to_owned(),
        ));
    }
    if request
        .response_projection
        .as_ref()
        .is_some_and(|projection| !projection.is_empty())
    {
        return Err(SemanticQueryError::Invalid(
            "Wave-5 response projection is outside the accepted minimal subset".to_owned(),
        ));
    }
    Ok(ValidatedSemanticRequest {
        request_digest: b3(&canonical_bytes),
        request,
        canonical_bytes,
    })
}

fn query_sql(query: &SemanticQueryClause) -> Result<String, SemanticQueryError> {
    let limit = query.limit.unwrap_or(QueryLimit {
        first: 100,
        offset: 0,
    });
    Ok(format!(
        "SELECT * FROM {} ORDER BY {} LIMIT {} OFFSET {}",
        query.request.table()?,
        query.request.order_key()?,
        limit.first,
        limit.offset
    ))
}

/// Execute all clauses through the existing immutable DataFusion snapshot session.
///
/// # Errors
///
/// Returns an error when a generated semantic mapping is invalid, snapshot execution fails, or the
/// canonical response cannot be encoded within the registered limits.
#[allow(clippy::too_many_lines)] // One pinned session keeps all clause results and response identities snapshot-coherent.
pub async fn execute_request(
    session: &ServingQuerySession,
    validated: ValidatedSemanticRequest,
) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
    let manifest = session.snapshot_manifest();
    let mut results = Vec::with_capacity(validated.request.queries.len());
    let mut entities = BTreeMap::new();
    let mut facts = BTreeMap::new();
    for query in &validated.request.queries {
        let result = session.query(&query_sql(query)?).await?;
        let ids = response_ids(&result.batches, query.request)?;
        let (entity_ids, fact_ids) = match query.request {
            QueryForm::FindEntities => (ids, Vec::new()),
            QueryForm::RetrieveFacts | QueryForm::FollowRelationships => (Vec::new(), ids),
            _ => {
                return Err(SemanticQueryError::Invalid(
                    "inactive query form reached execution".to_owned(),
                ));
            }
        };
        for entity_id in &entity_ids {
            entities.insert(
                entity_id.clone(),
                BTreeMap::from([("entity_id".to_owned(), entity_id.clone())]),
            );
        }
        for fact_id in &fact_ids {
            facts.insert(
                fact_id.clone(),
                BTreeMap::from([("fact_id".to_owned(), fact_id.clone())]),
            );
        }
        let phrase = resolve_phrase(query.request, query.label.as_deref())?;
        let mut resolved_semantics = BTreeMap::from([
            ("table".to_owned(), query.request.table()?.to_owned()),
            (
                "order_key".to_owned(),
                query.request.order_key()?.to_owned(),
            ),
        ]);
        if let Some(phrase) = phrase {
            resolved_semantics.insert("phrase_id".to_owned(), phrase.phrase_id.to_owned());
            resolved_semantics.insert(
                "canonical_phrase".to_owned(),
                phrase.canonical_text.to_owned(),
            );
        }
        results.push(QueryResultRecord {
            query_id: query.query_id.clone(),
            request: query.request,
            execution_state: "SUCCEEDED",
            availability_state: "AVAILABLE",
            completeness_state: "COMPLETE",
            freshness_state: "CURRENT",
            limit_state: "NOT_APPLIED",
            dependency_state: "SATISFIED",
            resolved_semantics,
            entity_ids,
            fact_ids,
            path_ids: Vec::new(),
            group_ids: Vec::new(),
            source_context_ids: manifest
                .body
                .contexts
                .records
                .iter()
                .map(|record| record.analysis_context_id.clone())
                .collect(),
            coverage: BTreeMap::from([(
                "returned_rows".to_owned(),
                u64::try_from(result.artifact.output_row_count).unwrap_or(u64::MAX),
            )]),
            errors: Vec::new(),
            notices: Vec::new(),
            output_row_count: result.artifact.output_row_count,
            result_checksum: result.artifact.result_checksum,
        });
    }
    let snapshot = snapshot_response(&manifest);
    let source_contexts = manifest
        .body
        .contexts
        .records
        .iter()
        .map(|record| {
            (
                record.analysis_context_id.clone(),
                BTreeMap::from([
                    (
                        "analysis_context_id".to_owned(),
                        record.analysis_context_id.clone(),
                    ),
                    (
                        "context_manifest_digest".to_owned(),
                        record.context_manifest_digest.clone(),
                    ),
                ]),
            )
        })
        .collect();
    let response = SemanticQueryResponse {
        specification: "composable semantic CPG fact query response",
        version: VERSION,
        semantic_request_id: validated.request.semantic_request_id,
        execution_state: "SUCCEEDED",
        availability_state: "AVAILABLE",
        completeness_state: "COMPLETE",
        freshness_state: "CURRENT",
        limit_state: "NOT_APPLIED",
        successful_query_count: results.len(),
        failed_query_count: 0,
        not_executed_dependency_count: 0,
        snapshot,
        entities,
        facts,
        paths: BTreeMap::new(),
        groups: BTreeMap::new(),
        source_contexts,
        query_results: results,
        errors: Vec::new(),
    };
    let value = serde_json::to_value(&response)
        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?;
    let canonical_bytes = canonicalize_value(&value)?;
    Ok(ExecutedSemanticResponse {
        response_digest: b3(&canonical_bytes),
        response,
        canonical_bytes,
    })
}

fn response_ids(
    batches: &[arrow_array::RecordBatch],
    form: QueryForm,
) -> Result<Vec<String>, SemanticQueryError> {
    let column_name = form.order_key()?;
    let prefix = match form {
        QueryForm::FindEntities => "entity:unknown:",
        QueryForm::RetrieveFacts => "fact:property:",
        QueryForm::FollowRelationships => "fact:relation:",
        _ => {
            return Err(SemanticQueryError::Invalid(
                "inactive query form reached response decoding".to_owned(),
            ));
        }
    };
    let mut ids = Vec::new();
    for batch in batches {
        let index = batch
            .schema()
            .index_of(column_name)
            .map_err(|_| SemanticQueryError::Invalid("result identity column is absent".into()))?;
        let values = batch
            .column(index)
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .ok_or_else(|| {
                SemanticQueryError::Invalid("result identity column is not Id16".into())
            })?;
        for row in 0..values.len() {
            if values.is_null(row) || values.value(row).len() != 16 {
                return Err(SemanticQueryError::Invalid(
                    "result identity is null or has invalid width".into(),
                ));
            }
            let raw: [u8; 16] = values.value(row).try_into().map_err(|_| {
                SemanticQueryError::Invalid("result identity has invalid width".into())
            })?;
            ids.push(format!("{prefix}{}", hex_id(raw)));
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub(crate) fn snapshot_response(
    manifest: &crate::snapshot::ServingSnapshotManifest,
) -> SemanticSnapshotResponse {
    let mut versions = crate::identity::semantic_fingerprint(
        crate::identity::SemanticFingerprintDomain::SnapshotBaseTableVersions,
    );
    for table in &manifest.body.base_publication.tables {
        versions.update(&table.table_code.to_be_bytes());
        versions.update(&table.delta_version.to_be_bytes());
        versions.update(table.schema_digest.as_bytes());
    }
    let bundle_version = |identity: &str| {
        identity
            .rsplit_once(':')
            .map_or_else(|| identity.to_owned(), |(_, version)| version.to_owned())
    };
    SemanticSnapshotResponse {
        snapshot_id: manifest.snapshot_id.clone(),
        workspace_id: manifest.body.workspace_id.clone(),
        repository_id: manifest.body.repository_id.clone(),
        worktree_id: manifest.body.worktree_id.clone(),
        source_generation: manifest.body.source.source_generation,
        source_inventory_digest: manifest.body.source.inventory_digest.clone(),
        durable_base_publication: manifest.body.base_publication.publication_id.clone(),
        base_table_version_digest: crate::integrity::frame_digest(versions.finalize()),
        overlay_generation: manifest.body.overlay.overlay_generation,
        overlay_checksum: manifest.body.overlay.overlay_digest.clone(),
        analysis_context_set_id: manifest.body.contexts.context_set_id.clone(),
        analysis_context_ids: manifest
            .body
            .contexts
            .records
            .iter()
            .map(|record| record.analysis_context_id.clone())
            .collect(),
        freshness_state: "CURRENT",
        source_trust_state: manifest.body.source.source_trust_state.clone(),
        event_stream_health: manifest.body.source.event_stream_health.clone(),
        git_acceleration_status: manifest.body.source.git_acceleration_status.clone(),
        git_operation_summary: None,
        pending_update_count: manifest
            .body
            .source
            .admitted_event_sequence
            .saturating_sub(manifest.body.source.reconciled_event_sequence),
        ontology_version: bundle_version(&manifest.body.bundles.ontology_bundle_id),
        schema_bundle_version: bundle_version(&manifest.body.bundles.schema_bundle_id),
        provider_bundle_version: bundle_version(&manifest.body.bundles.provider_bundle_id),
        derivation_bundle_version: bundle_version(&manifest.body.bundles.derivation_bundle_id),
        query_language_version: bundle_version(&manifest.body.bundles.query_language_bundle_id),
        capability_summaries: Vec::new(),
        diagnostic_references: Vec::new(),
    }
}

fn hex_id(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(32);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Vec<u8> {
        br#"{
          "specification":"composable semantic CPG fact query",
          "version":"1.3",
          "semantic_request_id":"gate-b",
          "workspace_id":"workspace:00000000000000000000000000000000",
          "freshness_policy":"best_available_snapshot",
          "queries":[
            {"query_id":"q1","request":"find code entities","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}},
            {"query_id":"q2","request":"retrieve facts about code","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}},
            {"query_id":"q3","request":"follow code relationships","label":null,"input":null,"where":null,"limit":{"first":10,"offset":0}}
          ],
          "response_projection":null,
          "cost_budget":{"maximum_rows":30}
        }"#
        .to_vec()
    }

    #[test]
    fn wp38_behavioral_acceptance() {
        let validated = validate_request(&request()).unwrap();
        assert_eq!(validated.request.queries.len(), 3);
        assert!(validated.request_digest.starts_with("b3:"));
        assert_eq!(
            query_sql(&validated.request.queries[0]).unwrap(),
            "SELECT * FROM entities ORDER BY entity_id LIMIT 10 OFFSET 0"
        );
    }

    #[test]
    fn wp38_negative_zero_state() {
        let unknown = String::from_utf8(request()).unwrap().replace(
            "\"response_projection\":null",
            "\"response_projection\":null,\"surprise\":true",
        );
        assert!(validate_request(unknown.as_bytes()).is_err());
        let duplicate = String::from_utf8(request())
            .unwrap()
            .replace("\"query_id\":\"q2\"", "\"query_id\":\"q1\"");
        assert!(validate_request(duplicate.as_bytes()).is_err());
        let over_budget = String::from_utf8(request())
            .unwrap()
            .replace("\"maximum_rows\":30", "\"maximum_rows\":29");
        assert!(validate_request(over_budget.as_bytes()).is_err());
        let incompatible_phrase = String::from_utf8(request()).unwrap().replace(
            "\"query_id\":\"q1\",\"request\":\"find code entities\",\"label\":null",
            "\"query_id\":\"q1\",\"request\":\"find code entities\",\"label\":\"call targets\"",
        );
        assert!(validate_request(incompatible_phrase.as_bytes()).is_err());
    }

    #[test]
    fn wp38_structural_acceptance() {
        let phrase = resolve_phrase(QueryForm::FindEntities, Some("syntax nodes"))
            .unwrap()
            .unwrap();
        assert_eq!(phrase.phrase_id, "Q51_SYNTAX_NODES");
        assert_eq!(phrase.plan_node_kind, "find-entities");
        let alias = resolve_phrase(QueryForm::FindEntities, Some("syntax occurrences"))
            .unwrap()
            .unwrap();
        assert_eq!(alias.phrase_id, phrase.phrase_id);
    }

    #[test]
    fn wp38_operational_acceptance() {
        let first = validate_request(&request()).unwrap();
        let second = validate_request(&first.canonical_bytes).unwrap();
        assert_eq!(first.canonical_bytes, second.canonical_bytes);
        assert_eq!(first.request_digest, second.request_digest);
        assert_eq!(
            first
                .request
                .queries
                .iter()
                .map(|query| query.request)
                .collect::<Vec<_>>(),
            vec![
                QueryForm::FindEntities,
                QueryForm::RetrieveFacts,
                QueryForm::FollowRelationships,
            ]
        );
    }
}
