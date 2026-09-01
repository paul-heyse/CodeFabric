//! Released semantic-query wire forms, parser products, and compatibility descriptors.
#![allow(clippy::match_same_arms)]
use std::collections::BTreeMap;

use thiserror::Error;

use crate::contracts::jcs::{CanonicalJsonError, canonicalize_slice, canonicalize_value};
use crate::registries::{FRESHNESS_STATE_VALUES, FreshnessState, registry_state_name};
use serde::{Deserialize, Serialize};

const MAX_REQUEST_BYTES: usize = 256 * 1024;

/// Freshness choice carried by the released request envelope.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessPolicy {
    CurrentRequired,
    WaitForCurrent,
    BestAvailableSnapshot,
    AwaitLatest,
    RequireCurrentForTargets,
    RequireSourceCurrent,
    RequireSemanticCurrent,
}

impl Serialize for FreshnessState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let name = registry_state_name(FRESHNESS_STATE_VALUES, *self as u16)
            .expect("registered freshness state and wire projection are one authority");
        serializer.serialize_str(name)
    }
}

/// Strict normalized DTO for the released v2.0 semantic-query envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticQueryRequest {
    pub specification: String,
    pub version: String,
    pub semantic_request_id: String,
    pub workspace_id: String,
    pub codebase: Option<String>,
    pub languages: Vec<String>,
    /// Lossless RFC/JCS canonical JSON UTF-8 operands.
    ///
    /// These strings are opaque authorization values. Semantic programs must not parse them or
    /// infer additional scope from their JSON structure.
    pub source_boundaries: Vec<String>,
    pub analysis_context_mode: Option<String>,
    pub analysis_context_ids: Vec<String>,
    pub representations: Vec<String>,
    pub external_entity_policy: Option<String>,
    pub freshness_policy: FreshnessPolicy,
    pub freshness_target_scope: Option<String>,
    pub freshness_deadline_ms: Option<u64>,
    pub queries: Vec<SemanticQueryClause>,
}

/// One role in the sole compiled v2.0 authorization-scope contract.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CompiledV20ScopeRole {
    WorkspaceId,
    Codebase,
    Language,
    SourceBoundary,
    AnalysisContextMode,
    AnalysisContextId,
    Representation,
    ExternalEntityPolicy,
}

/// One closed scope relation and its application-owned authorization operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledV20ScopeDefinition {
    pub role: CompiledV20ScopeRole,
    pub scope_id: &'static str,
    pub authorization_input_id: &'static str,
    pub minimum_values: usize,
    pub maximum_values: usize,
}

/// Exhaustive scope authority shared by the recipe, ingress, and authorization ports.
///
/// Specification/version validation and freshness admission deliberately do not appear here:
/// neither is a child-authorization scope relation.
pub(crate) const COMPILED_V2_0_SCOPE_DEFINITIONS: [CompiledV20ScopeDefinition; 8] = [
    CompiledV20ScopeDefinition {
        role: CompiledV20ScopeRole::WorkspaceId,
        scope_id: "scope.workspace-id",
        authorization_input_id: "authorization.workspace",
        minimum_values: 1,
        maximum_values: 1,
    },
    CompiledV20ScopeDefinition {
        role: CompiledV20ScopeRole::Codebase,
        scope_id: "scope.codebase",
        authorization_input_id: "authorization.codebase",
        minimum_values: 0,
        maximum_values: 1,
    },
    CompiledV20ScopeDefinition {
        role: CompiledV20ScopeRole::Language,
        scope_id: "scope.language",
        authorization_input_id: "authorization.language",
        minimum_values: 0,
        maximum_values: 32,
    },
    CompiledV20ScopeDefinition {
        role: CompiledV20ScopeRole::SourceBoundary,
        scope_id: "scope.source-boundary",
        authorization_input_id: "authorization.source-boundary",
        minimum_values: 0,
        maximum_values: 256,
    },
    CompiledV20ScopeDefinition {
        role: CompiledV20ScopeRole::AnalysisContextMode,
        scope_id: "scope.analysis-context-mode",
        authorization_input_id: "authorization.analysis-context-mode",
        minimum_values: 0,
        maximum_values: 1,
    },
    CompiledV20ScopeDefinition {
        role: CompiledV20ScopeRole::AnalysisContextId,
        scope_id: "scope.analysis-context-id",
        authorization_input_id: "authorization.analysis-context",
        minimum_values: 0,
        maximum_values: 256,
    },
    CompiledV20ScopeDefinition {
        role: CompiledV20ScopeRole::Representation,
        scope_id: "scope.representation",
        authorization_input_id: "authorization.representation",
        minimum_values: 0,
        maximum_values: 64,
    },
    CompiledV20ScopeDefinition {
        role: CompiledV20ScopeRole::ExternalEntityPolicy,
        scope_id: "scope.external-entity-policy",
        authorization_input_id: "authorization.external-entity-policy",
        minimum_values: 0,
        maximum_values: 1,
    },
];

impl SemanticQueryRequest {
    /// Return the lossless text operands for one compiled v2.0 scope role.
    #[must_use]
    pub(crate) fn compiled_v2_0_scope_operands(&self, role: CompiledV20ScopeRole) -> Vec<&str> {
        match role {
            CompiledV20ScopeRole::WorkspaceId => vec![self.workspace_id.as_str()],
            CompiledV20ScopeRole::Codebase => self.codebase.iter().map(String::as_str).collect(),
            CompiledV20ScopeRole::Language => self.languages.iter().map(String::as_str).collect(),
            CompiledV20ScopeRole::SourceBoundary => {
                self.source_boundaries.iter().map(String::as_str).collect()
            }
            CompiledV20ScopeRole::AnalysisContextMode => self
                .analysis_context_mode
                .iter()
                .map(String::as_str)
                .collect(),
            CompiledV20ScopeRole::AnalysisContextId => self
                .analysis_context_ids
                .iter()
                .map(String::as_str)
                .collect(),
            CompiledV20ScopeRole::Representation => {
                self.representations.iter().map(String::as_str).collect()
            }
            CompiledV20ScopeRole::ExternalEntityPolicy => self
                .external_entity_policy
                .iter()
                .map(String::as_str)
                .collect(),
        }
    }
}

/// Strictly decoded request together with its canonical bytes and content digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedSemanticRequest {
    pub request: SemanticQueryRequest,
    pub canonical_bytes: Vec<u8>,
    pub request_digest: String,
}

/// Stable failures at the released semantic-query ingress boundary.
#[derive(Debug, Error)]
pub enum SemanticQueryError {
    #[error("INVALID_REQUEST_SCHEMA:SEMANTIC_QUERY_INVALID:{0}")]
    Invalid(String),
    #[error("{code}:{phase}:{pointer}:{message}")]
    Phase {
        code: &'static str,
        phase: &'static str,
        pointer: String,
        message: String,
    },
    #[error(transparent)]
    Canonical(#[from] CanonicalJsonError),
}

/// Strictly decode and canonicalize one released semantic request.
///
/// This function assigns no compiler, program, or execution authority.
///
/// # Errors
///
/// Returns an error for oversized, non-canonical, or schema-invalid JSON.
pub fn parse_request(bytes: &[u8]) -> Result<ParsedSemanticRequest, SemanticQueryError> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(SemanticQueryError::Invalid(
            "request exceeds maximum bytes".to_owned(),
        ));
    }
    let canonical_bytes = canonicalize_slice(bytes)?;
    let identity: serde_json::Value = serde_json::from_slice(&canonical_bytes)
        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?;
    let version = identity
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            SemanticQueryError::Invalid("request version must be a string".to_owned())
        })?;
    let request = match version {
        "2.0" => translate_v2_request(&canonical_bytes)?,
        other => {
            return Err(SemanticQueryError::Invalid(format!(
                "unsupported semantic request version {other}"
            )));
        }
    };
    Ok(ParsedSemanticRequest {
        request,
        request_digest: crate::integrity::framed_digest(&canonical_bytes),
        canonical_bytes,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticQueryRequestV2Wire {
    specification: String,
    version: String,
    semantic_request_id: String,
    scope: SemanticQueryScopeV2Wire,
    freshness: SemanticQueryFreshnessV2Wire,
    #[serde(default, rename = "defaults")]
    _defaults: BTreeMap<String, serde_json::Value>,
    queries: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticQueryScopeV2Wire {
    workspace_id: String,
    #[serde(default)]
    codebase: Option<String>,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    source_boundaries: Vec<serde_json::Value>,
    #[serde(default)]
    analysis_contexts: Option<SemanticAnalysisContextsV2Wire>,
    #[serde(default)]
    representations: Vec<String>,
    #[serde(default)]
    external_entities: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticAnalysisContextsV2Wire {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    context_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticQueryFreshnessV2Wire {
    policy: String,
    #[serde(default)]
    target_scope: Option<String>,
    #[serde(default)]
    deadline_ms: Option<u64>,
}

fn translate_v2_request(bytes: &[u8]) -> Result<SemanticQueryRequest, SemanticQueryError> {
    let wire: SemanticQueryRequestV2Wire = serde_json::from_slice(bytes)
        .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?;
    if wire.version != "2.0" {
        return Err(SemanticQueryError::Invalid(
            "v2 request translator received a different version".to_owned(),
        ));
    }
    let freshness_policy = match wire.freshness.policy.as_str() {
        "best_available_snapshot" => FreshnessPolicy::BestAvailableSnapshot,
        "await_latest" => FreshnessPolicy::AwaitLatest,
        "require_current_for_targets" => FreshnessPolicy::RequireCurrentForTargets,
        "require_source_current" => FreshnessPolicy::RequireSourceCurrent,
        "require_semantic_current" => FreshnessPolicy::RequireSemanticCurrent,
        other => {
            return Err(SemanticQueryError::Invalid(format!(
                "unsupported v2 freshness policy {other}"
            )));
        }
    };
    let queries = wire
        .queries
        .into_iter()
        .map(translate_v2_clause)
        .collect::<Result<Vec<_>, _>>()?;
    let analysis_contexts = wire.scope.analysis_contexts.unwrap_or_default();
    let source_boundaries = wire
        .scope
        .source_boundaries
        .iter()
        .map(|boundary| {
            String::from_utf8(canonicalize_value(boundary)?).map_err(|error| {
                SemanticQueryError::Invalid(format!(
                    "canonical source boundary is not UTF-8: {error}"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let request = SemanticQueryRequest {
        specification: wire.specification,
        version: wire.version,
        semantic_request_id: wire.semantic_request_id,
        workspace_id: wire.scope.workspace_id,
        codebase: wire.scope.codebase,
        languages: wire.scope.languages,
        source_boundaries,
        analysis_context_mode: analysis_contexts.mode,
        analysis_context_ids: analysis_contexts.context_ids,
        representations: wire.scope.representations,
        external_entity_policy: wire.scope.external_entities,
        freshness_policy,
        freshness_target_scope: wire.freshness.target_scope,
        freshness_deadline_ms: wire.freshness.deadline_ms,
        queries,
    };
    for definition in COMPILED_V2_0_SCOPE_DEFINITIONS {
        let observed = request.compiled_v2_0_scope_operands(definition.role).len();
        if observed < definition.minimum_values || observed > definition.maximum_values {
            return Err(SemanticQueryError::Invalid(format!(
                "scope {} has {observed} values; expected {}..={}",
                definition.scope_id, definition.minimum_values, definition.maximum_values
            )));
        }
    }
    Ok(request)
}

fn translate_v2_clause(
    value: serde_json::Value,
) -> Result<SemanticQueryClause, SemanticQueryError> {
    let mut fields = value.as_object().cloned().ok_or_else(|| {
        SemanticQueryError::Invalid("v2 query block must be an object".to_owned())
    })?;
    let request = take_required_string(&mut fields, "request")?;
    let query_id = take_required_string(&mut fields, "query_id")?;
    let label = take_optional_string(&mut fields, "label")?;
    let return_spec = take_return_spec(&mut fields)?;
    let mut where_conditions = take_string_values(&mut fields, "where")?;
    for common in ["on_ambiguity", "on_unavailable", "extensions"] {
        fields.remove(common);
    }

    let clause = match request.as_str() {
        "find code entities" => SemanticQueryClause::FindEntities {
            query_id,
            label,
            looking_for: take_required_string(&mut fields, "looking_for")?,
            within: take_references(&mut fields, "within")?,
            where_conditions,
            return_spec,
        },
        "retrieve facts about code" => SemanticQueryClause::RetrieveFacts {
            query_id,
            label,
            about: take_references(&mut fields, "about")?,
            facts: take_required_string_array(&mut fields, "facts")?,
            at: take_optional_string(&mut fields, "at")?,
            where_conditions,
            return_spec,
        },
        "follow code relationships" => {
            let distance = take_optional_scalar_string(&mut fields, "distance")?;
            SemanticQueryClause::FollowRelationships {
                query_id,
                label,
                starting_from: take_references(&mut fields, "starting_from")?,
                relationship: take_required_string(&mut fields, "relationship")?,
                direction: take_optional_string(&mut fields, "direction")?,
                distance,
                stop_when: take_string_values(&mut fields, "stop_when")?,
                where_conditions,
                return_spec,
            }
        }
        "find connecting fact paths" => SemanticQueryClause::FindPaths {
            query_id,
            label,
            starting_from: take_references(&mut fields, "from")?,
            ending_at: take_references(&mut fields, "to")?,
            through: take_required_string_array(&mut fields, "using")?,
            path_policy: take_required_string(&mut fields, "path_policy")?,
            direction: take_optional_string(&mut fields, "direction")?,
            maximum_length: take_optional_usize(&mut fields, "maximum_length")?,
            where_conditions,
            return_spec,
        },
        "match a code fact pattern" => {
            let pattern = fields.remove("pattern").ok_or_else(|| {
                SemanticQueryError::Invalid("v2 pattern query lacks pattern".to_owned())
            })?;
            let (bindings, relationships, pattern_constraints) = translate_v2_pattern(pattern)?;
            where_conditions.extend(pattern_constraints);
            SemanticQueryClause::MatchPattern {
                query_id,
                label,
                bindings,
                relationships,
                where_conditions,
                return_spec,
            }
        }
        "combine result sets" => SemanticQueryClause::CombineResults {
            query_id,
            label,
            inputs: take_prior_results(&mut fields, "inputs")?,
            combination: take_required_string(&mut fields, "operation")?,
            identity: take_optional_string(&mut fields, "identity")?,
            preserve_origin: take_optional_string(&mut fields, "preserve_origin")?,
            return_spec,
        },
        "summarize objective facts" => SemanticQueryClause::SummarizeFacts {
            query_id,
            label,
            input: take_references(&mut fields, "about")?,
            summaries: vec![take_required_string(&mut fields, "measure")?],
            group_by: take_required_string_array(&mut fields, "group_by")?,
            include_support: take_optional_string(&mut fields, "include_support")?,
            where_conditions,
            return_spec,
        },
        "retrieve source and syntax context" => SemanticQueryClause::RetrieveSourceContext {
            query_id,
            label,
            for_inputs: take_references(&mut fields, "about")?,
            context: vec![take_required_string(&mut fields, "context")?],
            text_handling: Some("lossless UTF-8 else bytes".to_owned()),
            where_conditions,
            return_spec,
        },
        other => {
            return Err(SemanticQueryError::Invalid(format!(
                "unsupported v2 query form {other}"
            )));
        }
    };
    if !fields.is_empty() {
        return Err(SemanticQueryError::Invalid(format!(
            "unsupported fields in v2 {request} block: {:?}",
            fields.keys().collect::<Vec<_>>()
        )));
    }
    Ok(clause)
}

fn take_required_string(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<String, SemanticQueryError> {
    fields
        .remove(name)
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| SemanticQueryError::Invalid(format!("{name} must be a string")))
}

fn take_optional_string(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<Option<String>, SemanticQueryError> {
    match fields.remove(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(SemanticQueryError::Invalid(format!(
            "{name} must be a string or null"
        ))),
    }
}

fn take_optional_scalar_string(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<Option<String>, SemanticQueryError> {
    match fields.remove(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(value)) => Ok(Some(value)),
        Some(serde_json::Value::Number(value)) => Ok(Some(value.to_string())),
        Some(_) => Err(SemanticQueryError::Invalid(format!(
            "{name} must be a scalar or null"
        ))),
    }
}

fn take_optional_usize(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<Option<usize>, SemanticQueryError> {
    match fields.remove(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(value)) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| SemanticQueryError::Invalid(format!("{name} is outside usize"))),
        Some(_) => Err(SemanticQueryError::Invalid(format!(
            "{name} must be an unsigned integer or null"
        ))),
    }
}

fn take_required_string_array(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<Vec<String>, SemanticQueryError> {
    let value = fields.remove(name).ok_or_else(|| {
        SemanticQueryError::Invalid(format!("{name} must be an array of strings"))
    })?;
    string_array(value, name)
}

fn take_string_values(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<Vec<String>, SemanticQueryError> {
    let Some(value) = fields.remove(name) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| SemanticQueryError::Invalid(format!("{name} must be an array")))?;
    values
        .iter()
        .map(|value| match value {
            serde_json::Value::String(value) => Ok(value.clone()),
            other => serde_json::to_string(other)
                .map_err(|error| SemanticQueryError::Invalid(error.to_string())),
        })
        .collect()
}

fn string_array(value: serde_json::Value, name: &str) -> Result<Vec<String>, SemanticQueryError> {
    value
        .as_array()
        .ok_or_else(|| SemanticQueryError::Invalid(format!("{name} must be an array")))?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                SemanticQueryError::Invalid(format!("{name} must contain only strings"))
            })
        })
        .collect()
}

fn take_return_spec(
    fields: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<Option<ReturnSpec>, SemanticQueryError> {
    fields
        .remove("return")
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|error| SemanticQueryError::Invalid(error.to_string()))
        })
        .transpose()
}

fn take_references(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<Vec<SemanticReference>, SemanticQueryError> {
    let value = fields.remove(name).unwrap_or_else(|| serde_json::json!([]));
    value
        .as_array()
        .ok_or_else(|| SemanticQueryError::Invalid(format!("{name} must be an array")))?
        .iter()
        .cloned()
        .map(translate_v2_reference)
        .collect()
}

fn translate_v2_reference(
    value: serde_json::Value,
) -> Result<SemanticReference, SemanticQueryError> {
    if let Some(value) = value.as_str() {
        return Ok(if value.starts_with("entity:") {
            SemanticReference::Entity {
                entity_id: value.to_owned(),
            }
        } else if value.starts_with("fact:") {
            SemanticReference::Fact {
                fact_id: value.to_owned(),
            }
        } else {
            SemanticReference::Phrase(value.to_owned())
        });
    }
    let object = value.as_object().ok_or_else(|| {
        SemanticQueryError::Invalid("semantic reference must be a string or object".to_owned())
    })?;
    if let Some(entity_id) = object.get("entity_id").and_then(serde_json::Value::as_str) {
        return Ok(SemanticReference::Entity {
            entity_id: entity_id.to_owned(),
        });
    }
    if let Some(fact_id) = object.get("fact_id").and_then(serde_json::Value::as_str) {
        return Ok(SemanticReference::Fact {
            fact_id: fact_id.to_owned(),
        });
    }
    if let Some(phrase) = object
        .get("semantic_reference")
        .and_then(serde_json::Value::as_str)
    {
        return Ok(SemanticReference::Phrase(phrase.to_owned()));
    }
    if object.contains_key("results_of") {
        let reference: PriorResultReference = serde_json::from_value(value)
            .map_err(|error| SemanticQueryError::Invalid(error.to_string()))?;
        return Ok(SemanticReference::PriorResult(reference));
    }
    if object.contains_key("source_location") {
        return serde_json::to_string(&value)
            .map(SemanticReference::Phrase)
            .map_err(|error| SemanticQueryError::Invalid(error.to_string()));
    }
    Err(SemanticQueryError::Invalid(
        "semantic reference object has no released discriminator".to_owned(),
    ))
}

fn take_prior_results(
    fields: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<Vec<PriorResultReference>, SemanticQueryError> {
    let value = fields
        .remove(name)
        .ok_or_else(|| SemanticQueryError::Invalid(format!("{name} must be an array")))?;
    serde_json::from_value(value).map_err(|error| SemanticQueryError::Invalid(error.to_string()))
}

fn translate_v2_pattern(
    value: serde_json::Value,
) -> Result<(Vec<PatternBinding>, Vec<PatternRelationship>, Vec<String>), SemanticQueryError> {
    let object = value
        .as_object()
        .ok_or_else(|| SemanticQueryError::Invalid("pattern must be an object".to_owned()))?;
    let nodes = object
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| SemanticQueryError::Invalid("pattern.nodes must be an array".to_owned()))?;
    let mut bindings = Vec::with_capacity(nodes.len());
    for node in nodes {
        let node = node.as_object().ok_or_else(|| {
            SemanticQueryError::Invalid("pattern node must be an object".to_owned())
        })?;
        let binding = node
            .get("binding")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SemanticQueryError::Invalid("pattern binding is required".to_owned()))?;
        let looking_for = node
            .get("semantic_kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                SemanticQueryError::Invalid("pattern semantic_kind is required".to_owned())
            })?;
        let within = node
            .get("module_id")
            .and_then(serde_json::Value::as_str)
            .map(|entity_id| SemanticReference::Entity {
                entity_id: entity_id.to_owned(),
            });
        let where_conditions = node
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(|name| vec![format!("name={name}")])
            .unwrap_or_default();
        bindings.push(PatternBinding {
            name: binding.to_owned(),
            looking_for: looking_for.to_owned(),
            within,
            where_conditions,
        });
    }
    let relationships = object
        .get("facts")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|fact| {
            let fact = fact.as_object()?;
            Some(PatternRelationship {
                from: fact.get("from")?.as_str()?.to_owned(),
                to: fact.get("to")?.as_str()?.to_owned(),
                relationship: fact.get("relationship")?.as_str()?.to_owned(),
                direction: fact
                    .get("direction")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                distance: fact.get("distance").map(|value| {
                    value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_owned)
                }),
            })
        })
        .collect::<Vec<_>>();
    let mut constraints = Vec::new();
    for name in ["alternatives", "scoped_negation"] {
        if let Some(values) = object.get(name).and_then(serde_json::Value::as_array) {
            constraints.extend(values.iter().map(|value| format!("{name}={value}")));
        }
    }
    let allowed = ["nodes", "facts", "alternatives", "scoped_negation"];
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(SemanticQueryError::Invalid(
            "pattern contains an unsupported field".to_owned(),
        ));
    }
    Ok((bindings, relationships, constraints))
}

/// Released status/snapshot projection returned by the query service.
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
    pub freshness_state: FreshnessState,
    pub source_trust_state: String,
    pub event_stream_health: String,
    pub git_acceleration_status: String,
    pub git_operation_summary: Option<BTreeMap<String, String>>,
    pub pending_update_count: u64,
    /// Released v1.3 spelling retained only in this compatibility projection.
    pub ontology_version: String,
    pub schema_bundle_version: String,
    pub provider_bundle_version: String,
    pub derivation_bundle_version: String,
    pub query_language_version: String,
    pub capability_summaries: Vec<BTreeMap<String, String>>,
    pub diagnostic_references: Vec<String>,
}

/// Closed released request-form vocabulary.
///
/// This is a wire/parser contract, not a generated ontology registry and not execution
/// authority. Programmatic compilation still dispatches on the fully typed
/// [`SemanticQueryClause`] variant and never looks up a form by code or slug.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum QueryForm {
    FindEntities,
    RetrieveFacts,
    FollowRelationships,
    FindPaths,
    MatchPattern,
    CombineResults,
    SummarizeFacts,
    RetrieveSourceContext,
}

impl QueryForm {
    /// Released forms in stable presentation order.
    pub const ALL: [Self; 8] = [
        Self::FindEntities,
        Self::RetrieveFacts,
        Self::FollowRelationships,
        Self::FindPaths,
        Self::MatchPattern,
        Self::CombineResults,
        Self::SummarizeFacts,
        Self::RetrieveSourceContext,
    ];

    /// Exact released request discriminator.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::FindEntities => "find code entities",
            Self::RetrieveFacts => "retrieve facts about code",
            Self::FollowRelationships => "follow code relationships",
            Self::FindPaths => "find connecting fact paths",
            Self::MatchPattern => "match a code fact pattern",
            Self::CombineResults => "combine result sets",
            Self::SummarizeFacts => "summarize objective facts",
            Self::RetrieveSourceContext => "retrieve source and syntax context",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultRole {
    Entities,
    Facts,
    Paths,
    PatternBindings,
    Groups,
    Summary,
    SourceContexts,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PriorResultReference {
    pub results_of: String,
    pub select: ResultRole,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SemanticReference {
    Phrase(String),
    PriorResult(PriorResultReference),
    Entity { entity_id: String },
    Fact { fact_id: String },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnLimit {
    pub maximum_results: usize,
    pub per: Option<String>,
    pub when_exceeded: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnSpec {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    pub result_shape: Option<String>,
    #[serde(default)]
    pub group_by: Vec<String>,
    #[serde(default)]
    pub order_by: Vec<String>,
    pub deduplicate_by: Option<String>,
    pub supporting_facts: Option<String>,
    pub include_query_result: Option<bool>,
    pub limit: Option<ReturnLimit>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatternBinding {
    pub name: String,
    pub looking_for: String,
    pub within: Option<SemanticReference>,
    #[serde(default, rename = "where")]
    pub where_conditions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatternRelationship {
    pub from: String,
    pub to: String,
    pub relationship: String,
    pub direction: Option<String>,
    pub distance: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "request", deny_unknown_fields)]
pub enum SemanticQueryClause {
    #[serde(rename = "find code entities")]
    FindEntities {
        query_id: String,
        label: Option<String>,
        looking_for: String,
        #[serde(default)]
        within: Vec<SemanticReference>,
        #[serde(rename = "where")]
        #[serde(default)]
        where_conditions: Vec<String>,
        #[serde(rename = "return")]
        return_spec: Option<ReturnSpec>,
    },
    #[serde(rename = "retrieve facts about code")]
    RetrieveFacts {
        query_id: String,
        label: Option<String>,
        about: Vec<SemanticReference>,
        facts: Vec<String>,
        at: Option<String>,
        #[serde(rename = "where")]
        #[serde(default)]
        where_conditions: Vec<String>,
        #[serde(rename = "return")]
        return_spec: Option<ReturnSpec>,
    },
    #[serde(rename = "follow code relationships")]
    FollowRelationships {
        query_id: String,
        label: Option<String>,
        starting_from: Vec<SemanticReference>,
        relationship: String,
        direction: Option<String>,
        distance: Option<String>,
        #[serde(default)]
        stop_when: Vec<String>,
        #[serde(rename = "where")]
        #[serde(default)]
        where_conditions: Vec<String>,
        #[serde(rename = "return")]
        return_spec: Option<ReturnSpec>,
    },
    #[serde(rename = "find connecting fact paths")]
    FindPaths {
        query_id: String,
        label: Option<String>,
        starting_from: Vec<SemanticReference>,
        ending_at: Vec<SemanticReference>,
        through: Vec<String>,
        path_policy: String,
        direction: Option<String>,
        maximum_length: Option<usize>,
        #[serde(rename = "where")]
        #[serde(default)]
        where_conditions: Vec<String>,
        #[serde(rename = "return")]
        return_spec: Option<ReturnSpec>,
    },
    #[serde(rename = "match a code fact pattern")]
    MatchPattern {
        query_id: String,
        label: Option<String>,
        bindings: Vec<PatternBinding>,
        relationships: Vec<PatternRelationship>,
        #[serde(rename = "where")]
        #[serde(default)]
        where_conditions: Vec<String>,
        #[serde(rename = "return")]
        return_spec: Option<ReturnSpec>,
    },
    #[serde(rename = "combine result sets")]
    CombineResults {
        query_id: String,
        label: Option<String>,
        inputs: Vec<PriorResultReference>,
        combination: String,
        identity: Option<String>,
        preserve_origin: Option<String>,
        #[serde(rename = "return")]
        return_spec: Option<ReturnSpec>,
    },
    #[serde(rename = "summarize objective facts")]
    SummarizeFacts {
        query_id: String,
        label: Option<String>,
        input: Vec<SemanticReference>,
        summaries: Vec<String>,
        #[serde(default)]
        group_by: Vec<String>,
        include_support: Option<String>,
        #[serde(rename = "where")]
        #[serde(default)]
        where_conditions: Vec<String>,
        #[serde(rename = "return")]
        return_spec: Option<ReturnSpec>,
    },
    #[serde(rename = "retrieve source and syntax context")]
    RetrieveSourceContext {
        query_id: String,
        label: Option<String>,
        #[serde(rename = "for")]
        for_inputs: Vec<SemanticReference>,
        context: Vec<String>,
        text_handling: Option<String>,
        #[serde(rename = "where")]
        #[serde(default)]
        where_conditions: Vec<String>,
        #[serde(rename = "return")]
        return_spec: Option<ReturnSpec>,
    },
}

impl SemanticQueryClause {
    #[must_use]
    pub fn query_id(&self) -> &str {
        match self {
            Self::FindEntities { query_id, .. } => query_id,
            Self::RetrieveFacts { query_id, .. } => query_id,
            Self::FollowRelationships { query_id, .. } => query_id,
            Self::FindPaths { query_id, .. } => query_id,
            Self::MatchPattern { query_id, .. } => query_id,
            Self::CombineResults { query_id, .. } => query_id,
            Self::SummarizeFacts { query_id, .. } => query_id,
            Self::RetrieveSourceContext { query_id, .. } => query_id,
        }
    }
    #[must_use]
    pub const fn form(&self) -> QueryForm {
        match self {
            Self::FindEntities { .. } => QueryForm::FindEntities,
            Self::RetrieveFacts { .. } => QueryForm::RetrieveFacts,
            Self::FollowRelationships { .. } => QueryForm::FollowRelationships,
            Self::FindPaths { .. } => QueryForm::FindPaths,
            Self::MatchPattern { .. } => QueryForm::MatchPattern,
            Self::CombineResults { .. } => QueryForm::CombineResults,
            Self::SummarizeFacts { .. } => QueryForm::SummarizeFacts,
            Self::RetrieveSourceContext { .. } => QueryForm::RetrieveSourceContext,
        }
    }
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::FindEntities { label, .. } => label.as_deref(),
            Self::RetrieveFacts { label, .. } => label.as_deref(),
            Self::FollowRelationships { label, .. } => label.as_deref(),
            Self::FindPaths { label, .. } => label.as_deref(),
            Self::MatchPattern { label, .. } => label.as_deref(),
            Self::CombineResults { label, .. } => label.as_deref(),
            Self::SummarizeFacts { label, .. } => label.as_deref(),
            Self::RetrieveSourceContext { label, .. } => label.as_deref(),
        }
    }
    #[must_use]
    pub const fn output_role(&self) -> ResultRole {
        match self {
            Self::FindEntities { .. } => ResultRole::Entities,
            Self::RetrieveFacts { .. } => ResultRole::Facts,
            Self::FollowRelationships { .. } => ResultRole::Facts,
            Self::FindPaths { .. } => ResultRole::Paths,
            Self::MatchPattern { .. } => ResultRole::PatternBindings,
            Self::CombineResults { .. } => ResultRole::Groups,
            Self::SummarizeFacts { .. } => ResultRole::Summary,
            Self::RetrieveSourceContext { .. } => ResultRole::SourceContexts,
        }
    }
    #[must_use]
    pub fn maximum_results(&self) -> usize {
        let spec = match self {
            Self::FindEntities { return_spec, .. } => return_spec.as_ref(),
            Self::RetrieveFacts { return_spec, .. } => return_spec.as_ref(),
            Self::FollowRelationships { return_spec, .. } => return_spec.as_ref(),
            Self::FindPaths { return_spec, .. } => return_spec.as_ref(),
            Self::MatchPattern { return_spec, .. } => return_spec.as_ref(),
            Self::CombineResults { return_spec, .. } => return_spec.as_ref(),
            Self::SummarizeFacts { return_spec, .. } => return_spec.as_ref(),
            Self::RetrieveSourceContext { return_spec, .. } => return_spec.as_ref(),
        };
        spec.and_then(|value| value.limit.as_ref())
            .map_or(100, |limit| limit.maximum_results)
    }
    #[must_use]
    pub fn result_references(&self) -> Vec<&PriorResultReference> {
        let mut result = Vec::new();
        match self {
            Self::FindEntities { within, .. } => {
                for value in within {
                    if let SemanticReference::PriorResult(reference) = value {
                        result.push(reference);
                    }
                }
            }
            Self::RetrieveFacts { about, .. } => {
                for value in about {
                    if let SemanticReference::PriorResult(reference) = value {
                        result.push(reference);
                    }
                }
            }
            Self::FollowRelationships { starting_from, .. } => {
                for value in starting_from {
                    if let SemanticReference::PriorResult(reference) = value {
                        result.push(reference);
                    }
                }
            }
            Self::FindPaths {
                starting_from,
                ending_at,
                ..
            } => {
                for value in starting_from {
                    if let SemanticReference::PriorResult(reference) = value {
                        result.push(reference);
                    }
                }
                for value in ending_at {
                    if let SemanticReference::PriorResult(reference) = value {
                        result.push(reference);
                    }
                }
            }
            Self::MatchPattern { bindings, .. } => {
                for binding in bindings {
                    if let Some(SemanticReference::PriorResult(reference)) = &binding.within {
                        result.push(reference);
                    }
                }
            }
            Self::CombineResults { inputs, .. } => {
                result.extend(inputs.iter());
            }
            Self::SummarizeFacts { input, .. } => {
                for value in input {
                    if let SemanticReference::PriorResult(reference) = value {
                        result.push(reference);
                    }
                }
            }
            Self::RetrieveSourceContext { for_inputs, .. } => {
                for value in for_inputs {
                    if let SemanticReference::PriorResult(reference) = value {
                        result.push(reference);
                    }
                }
            }
        }
        result
    }
    #[must_use]
    pub fn direct_entity_ids(&self) -> Vec<&str> {
        self.semantic_references()
            .into_iter()
            .filter_map(|value| {
                if let SemanticReference::Entity { entity_id } = value {
                    Some(entity_id.as_str())
                } else {
                    None
                }
            })
            .collect()
    }
    #[must_use]
    pub fn direct_fact_ids(&self) -> Vec<&str> {
        self.semantic_references()
            .into_iter()
            .filter_map(|value| {
                if let SemanticReference::Fact { fact_id } = value {
                    Some(fact_id.as_str())
                } else {
                    None
                }
            })
            .collect()
    }
    #[must_use]
    pub fn semantic_references(&self) -> Vec<&SemanticReference> {
        let mut result = Vec::new();
        match self {
            Self::FindEntities { within, .. } => {
                result.extend(within.iter());
            }
            Self::RetrieveFacts { about, .. } => {
                result.extend(about.iter());
            }
            Self::FollowRelationships { starting_from, .. } => {
                result.extend(starting_from.iter());
            }
            Self::FindPaths {
                starting_from,
                ending_at,
                ..
            } => {
                result.extend(starting_from.iter());
                result.extend(ending_at.iter());
            }
            Self::MatchPattern { bindings, .. } => {
                result.extend(
                    bindings
                        .iter()
                        .filter_map(|binding| binding.within.as_ref()),
                );
            }
            Self::CombineResults { .. } => {}
            Self::SummarizeFacts { input, .. } => {
                result.extend(input.iter());
            }
            Self::RetrieveSourceContext { for_inputs, .. } => {
                result.extend(for_inputs.iter());
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST: &[u8] = br#"{
        "specification":"composable semantic CPG fact query",
        "version":"2.0",
        "semantic_request_id":"request:1",
        "scope":{
            "workspace_id":"workspace:00000000000000000000000000000000",
            "codebase":"codebase:current",
            "languages":["Rust","Python"],
            "source_boundaries":[{"root":"src","kind":"path"}],
            "analysis_contexts":{"mode":"explicit","context_ids":["analysis:one"]},
            "representations":["syntax","semantic"],
            "external_entities":"endpoint-only"
        },
        "freshness":{
            "policy":"best_available_snapshot",
            "target_scope":"semantic",
            "deadline_ms":2500
        },
        "queries":[{
            "request":"find code entities",
            "query_id":"q1",
            "looking_for":"syntax nodes",
            "within":[],
            "where":[],
            "return":{"limit":{"maximum_results":1}}
        }]
    }"#;

    #[test]
    fn released_request_parser_is_authority_neutral_and_canonical() {
        let parsed = parse_request(REQUEST).expect("released request must decode");
        assert_eq!(
            parsed.canonical_bytes,
            canonicalize_slice(REQUEST).expect("canonical request")
        );
        assert_eq!(parsed.request.queries.len(), 1);
        assert_eq!(parsed.request.codebase.as_deref(), Some("codebase:current"));
        assert_eq!(parsed.request.languages, ["Rust", "Python"]);
        assert_eq!(
            parsed.request.source_boundaries,
            [r#"{"kind":"path","root":"src"}"#]
        );
        assert_eq!(
            parsed.request.analysis_context_mode.as_deref(),
            Some("explicit")
        );
        assert_eq!(parsed.request.analysis_context_ids, ["analysis:one"]);
        assert_eq!(parsed.request.representations, ["syntax", "semantic"]);
        assert_eq!(
            parsed.request.external_entity_policy.as_deref(),
            Some("endpoint-only")
        );
        assert_eq!(
            parsed.request.freshness_policy,
            FreshnessPolicy::BestAvailableSnapshot
        );
        assert_eq!(
            parsed.request.freshness_target_scope.as_deref(),
            Some("semantic")
        );
        assert_eq!(parsed.request.freshness_deadline_ms, Some(2_500));
        assert_eq!(
            parsed.request_digest,
            crate::integrity::framed_digest(&parsed.canonical_bytes)
        );
    }

    #[test]
    fn released_request_parser_rejects_unreleased_fields() {
        let mut value: serde_json::Value = serde_json::from_slice(REQUEST).unwrap();
        value["ontology_candidate"] = serde_json::Value::String("candidate:old".to_owned());
        let bytes = crate::contracts::jcs::canonicalize_value(&value).unwrap();
        let error = parse_request(&bytes).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn released_request_parser_rejects_v1_3_instead_of_translating_legacy_globals() {
        let legacy = br#"{"freshness_policy":"current_required","queries":[],"semantic_request_id":"request:legacy","specification":"composable semantic CPG fact query","version":"1.3","workspace_id":"workspace:legacy"}"#;
        let error = parse_request(legacy).expect_err("v1.3 is not an operable request envelope");
        assert!(
            error
                .to_string()
                .contains("unsupported semantic request version 1.3")
        );
    }

    #[test]
    fn released_query_form_vocabulary_is_closed_without_a_generated_registry() {
        assert_eq!(
            QueryForm::ALL.map(QueryForm::slug),
            [
                "find code entities",
                "retrieve facts about code",
                "follow code relationships",
                "find connecting fact paths",
                "match a code fact pattern",
                "combine result sets",
                "summarize objective facts",
                "retrieve source and syntax context",
            ]
        );
    }
}
