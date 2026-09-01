//! Explicit unknown fact materialization for incomplete requested fact-family coverage.
//!
//! A known fact and an unavailable/partial requested family coexist in one query result. The
//! family remainder therefore becomes a first-class typed row instead of suppressing known rows
//! or turning missing provider output into an empty-result absence claim.

use std::sync::Arc;

use thiserror::Error;

use crate::identity::{
    IdentityDomain, decode_b3_digest, decode_public_id, decode_public_id_any_kind,
    derive_public_recipe_identity_with_kind,
};
use crate::identity_recipes::{self as recipes, RecipeValue};

/// Incomplete provider-coverage state that requires an explicit unknown fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownCoverageState {
    Unavailable,
    Partial,
}

impl UnknownCoverageState {
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Unavailable => "UNAVAILABLE",
            Self::Partial => "PARTIAL",
        }
    }

    #[must_use]
    pub const fn resolution(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Partial => "partial",
        }
    }
}

/// Complete issued identity and provenance inputs for one unknown-family materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitUnknownFactInput {
    pub workspace_id: Arc<str>,
    pub analysis_context_id: Arc<str>,
    pub subject_id: Arc<str>,
    pub requested_family: Arc<str>,
    pub property_kind_code: u16,
    pub canonical_value: Arc<str>,
    pub source_file_id: Arc<str>,
    pub source_content_digest: Arc<str>,
    pub producer_closure_id: Arc<str>,
    pub policy_identity: Arc<str>,
    pub reason: Arc<str>,
    pub coverage_state: UnknownCoverageState,
    pub producer_release: Arc<str>,
    pub source_generation: u64,
    pub input_set_id: Arc<str>,
    pub support_ids: Arc<[Arc<str>]>,
}

/// Typed explicit-unknown fact row ready for canonical Arrow materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitUnknownFact {
    pub fact_id: Arc<str>,
    pub identity_recipe: serde_json::Value,
    pub property_kind_code: u16,
    pub workspace_id: Arc<str>,
    pub analysis_context_id: Arc<str>,
    pub subject_id: Arc<str>,
    pub requested_family: Arc<str>,
    pub reason: Arc<str>,
    pub coverage_state: UnknownCoverageState,
    pub producer_id: Arc<str>,
    pub producer_release: Arc<str>,
    pub source_generation: u64,
    pub input_set_id: Arc<str>,
    pub support_ids: Arc<[Arc<str>]>,
}

/// Fail-closed explicit-unknown materialization errors.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ExplicitUnknownFactError {
    #[error("explicit unknown fact has an invalid required identity")]
    InvalidRequiredIdentity,
    #[error("explicit unknown fact has an invalid requested family")]
    InvalidRequestedFamily,
    #[error("explicit unknown fact canonical identity derivation failed")]
    CanonicalIdentity,
}

/// Materialize one typed unknown fact without suppressing any known family rows.
///
/// Identity is the released `PROPERTY_FACT` proposition `UNKNOWN_EFFECT(subject, family)`.
/// Mutable coverage state, reason, retryability, and provenance remain excluded observations.
///
/// # Errors
///
/// Rejects missing/unbounded identities or family labels before an Arrow row can be emitted.
pub fn materialize_explicit_unknown_fact(
    input: ExplicitUnknownFactInput,
) -> Result<ExplicitUnknownFact, ExplicitUnknownFactError> {
    if [
        &input.workspace_id,
        &input.analysis_context_id,
        &input.subject_id,
        &input.source_file_id,
        &input.source_content_digest,
        &input.input_set_id,
        &input.producer_closure_id,
        &input.policy_identity,
    ]
    .into_iter()
    .any(|value| !bounded(value, 256))
    {
        return Err(ExplicitUnknownFactError::InvalidRequiredIdentity);
    }
    if !bounded(&input.requested_family, 128)
        || input.reason.is_empty()
        || input.reason.len() > 512
        || input.property_kind_code == 0
        || !bounded(&input.canonical_value, 512)
    {
        return Err(ExplicitUnknownFactError::InvalidRequestedFamily);
    }
    let workspace_id = decode_public_id(IdentityDomain::Workspace, None, &input.workspace_id)
        .map_err(|_| ExplicitUnknownFactError::InvalidRequiredIdentity)?;
    let analysis_context_id = decode_public_id(
        IdentityDomain::AnalysisContext,
        None,
        &input.analysis_context_id,
    )
    .map_err(|_| ExplicitUnknownFactError::InvalidRequiredIdentity)?;
    let subject_id = decode_public_id_any_kind(IdentityDomain::Entity, &input.subject_id)
        .map_err(|_| ExplicitUnknownFactError::InvalidRequiredIdentity)?;
    decode_public_id(IdentityDomain::SourceFile, None, &input.source_file_id)
        .map_err(|_| ExplicitUnknownFactError::InvalidRequiredIdentity)?;
    decode_b3_digest(&input.source_content_digest)
        .map_err(|_| ExplicitUnknownFactError::InvalidRequiredIdentity)?;
    decode_public_id(IdentityDomain::ObjectiveInputSet, None, &input.input_set_id)
        .map_err(|_| ExplicitUnknownFactError::InvalidRequiredIdentity)?;
    input.support_ids.iter().try_for_each(|support_id| {
        decode_public_id_any_kind(IdentityDomain::RelationFact, support_id)
            .map(|_| ())
            .map_err(|_| ExplicitUnknownFactError::InvalidRequiredIdentity)
    })?;
    let record = recipes::property_fact(recipes::PropertyFactFields {
        workspace_id: RecipeValue::Id(workspace_id),
        analysis_context_id: RecipeValue::Id(analysis_context_id),
        property_kind_code: RecipeValue::Unsigned(input.property_kind_code.to_be_bytes().to_vec()),
        subject_entity_id: RecipeValue::Id(subject_id),
        canonical_value: RecipeValue::TaggedUnion(
            50,
            Box::new(RecipeValue::Utf8(input.canonical_value.to_string())),
        ),
    })
    .map_err(|_| ExplicitUnknownFactError::CanonicalIdentity)?;
    let identity = derive_public_recipe_identity_with_kind(
        record,
        Some("unknown-effect"),
        vec![
            ("workspace_id", serde_json::json!(input.workspace_id)),
            (
                "analysis_context_id",
                serde_json::json!(input.analysis_context_id),
            ),
            (
                "property_kind_code",
                serde_json::json!(input.property_kind_code),
            ),
            ("subject_entity_id", serde_json::json!(input.subject_id)),
            (
                "canonical_value",
                serde_json::json!({
                    "variant": 50,
                    "member_type": "UTF8",
                    "value": input.canonical_value,
                }),
            ),
        ],
        &[
            "coverage state",
            "coverage reason",
            "retryability",
            "source and producer provenance",
            "input-set and policy identity",
            "diagnostic evidence",
            "mutable coverage counters",
        ],
    )
    .map_err(|_| ExplicitUnknownFactError::CanonicalIdentity)?;
    let identity_recipe = identity.recipe_evidence();
    Ok(ExplicitUnknownFact {
        producer_id: Arc::from(format!("coverage:{}", input.requested_family)),
        fact_id: Arc::from(identity.public_id),
        identity_recipe,
        property_kind_code: input.property_kind_code,
        workspace_id: input.workspace_id,
        analysis_context_id: input.analysis_context_id,
        subject_id: input.subject_id,
        requested_family: input.requested_family,
        reason: input.reason,
        coverage_state: input.coverage_state,
        producer_release: input.producer_release,
        source_generation: input.source_generation,
        input_set_id: input.input_set_id,
        support_ids: input.support_ids,
    })
}

fn bounded(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.trim() == value && value.len() <= maximum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ExplicitUnknownFactInput {
        ExplicitUnknownFactInput {
            workspace_id: Arc::from(format!("workspace:{}", "00".repeat(16))),
            analysis_context_id: Arc::from(format!("context:{}", "11".repeat(16))),
            subject_id: Arc::from(format!("entity:function:{}", "44".repeat(16))),
            requested_family: Arc::from("effects"),
            property_kind_code: 2,
            canonical_value: Arc::from("effects"),
            source_file_id: Arc::from(format!("file:{}", "66".repeat(16))),
            source_content_digest: Arc::from(format!("b3:{}", "cc".repeat(32))),
            producer_closure_id: Arc::from("producer-closure:r1"),
            policy_identity: Arc::from("policy:r1"),
            reason: Arc::from("unsupported"),
            coverage_state: UnknownCoverageState::Unavailable,
            producer_release: Arc::from("r1"),
            source_generation: 2,
            input_set_id: Arc::from(format!("input-set:{}", "88".repeat(16))),
            support_ids: Arc::from([]),
        }
    }

    #[test]
    fn unknown_effect_is_a_domain_10_property_fact_known_answer() {
        let fact = materialize_explicit_unknown_fact(input()).expect("domain-10 unknown KAT");
        assert_eq!(
            fact.fact_id.as_ref(),
            "fact:unknown-effect:6419f535d7c0d1f7cfedf900eea465ff"
        );
        assert_eq!(
            fact.identity_recipe["record_domain"],
            serde_json::json!({"code": 10, "name": "PROPERTY_FACT"})
        );
        assert_eq!(fact.property_kind_code, 2);
        assert_eq!(
            fact.identity_recipe["digest"]["full_digest_hex"],
            "6419f535d7c0d1f7cfedf900eea465ff64d647ee868d6354d632ba2427c448db"
        );
        assert_eq!(
            fact.identity_recipe["fields"][4]["value"],
            serde_json::json!({
                "variant": 50,
                "member_type": "UTF8",
                "value": "effects"
            })
        );

        let mut mutable_evidence_changed = input();
        mutable_evidence_changed.coverage_state = UnknownCoverageState::Partial;
        mutable_evidence_changed.reason = Arc::from("retryable provider gap");
        mutable_evidence_changed.source_generation = 99;
        mutable_evidence_changed.input_set_id = Arc::from(format!("input-set:{}", "89".repeat(16)));
        assert_eq!(
            materialize_explicit_unknown_fact(mutable_evidence_changed)
                .unwrap()
                .fact_id,
            fact.fact_id
        );

        let mut proposition_changed = input();
        proposition_changed.canonical_value = Arc::from("allocations");
        assert_ne!(
            materialize_explicit_unknown_fact(proposition_changed)
                .unwrap()
                .fact_id,
            fact.fact_id
        );
    }
}
