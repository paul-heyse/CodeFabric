//! Authorization-checked source-span materialization for semantic query results.
//!
//! Fact visibility never implies source visibility. This boundary accepts one already pinned
//! source image and one canonical half-open entity span, independently checks the source grant,
//! and applies the semantic source-byte limit separately from the hard service output envelope.

use std::sync::Arc;

use thiserror::Error;

use crate::identity::{
    CanonicalPublicIdentity, IdentityDomain, decode_b3_digest, decode_public_id,
    decode_public_id_any_kind, derive_public_recipe_identity,
};
use crate::identity_recipes::{self as recipes, RecipeValue};

/// Canonical identity and byte coordinates retained by one source-context observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpanIdentity {
    pub entity_id: Arc<str>,
    pub workspace_id: Arc<str>,
    pub source_file_id: Arc<str>,
    pub content_digest: Arc<str>,
    pub byte_safe_path: Arc<str>,
    pub start_byte: usize,
    pub end_byte: usize,
    pub source_generation: u64,
}

/// Independent source-disclosure grant for one pinned workspace byte range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAccessGrant {
    pub source_access: bool,
    pub workspace_id: Arc<str>,
    pub authorized_start_byte: usize,
    pub authorized_end_byte: usize,
    pub authorization_scope: Arc<str>,
}

/// Request-local source material and its two independent output bounds.
#[derive(Clone, Debug)]
pub struct SourceContextMaterializationInput<'a> {
    pub span: SourceSpanIdentity,
    pub grant: SourceAccessGrant,
    pub analysis_context_id: Arc<str>,
    pub snapshot_id: Arc<str>,
    pub context_kind: Arc<str>,
    pub policy_identity: Arc<str>,
    pub source_bytes: &'a [u8],
    pub declared_byte_length: usize,
    pub explicit_source_byte_limit: usize,
    pub hard_output_byte_limit: usize,
}

/// Lossless source representation selected after applying the exact byte limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceContextContent {
    Text(Arc<str>),
    Bytes(Arc<[u8]>),
}

/// Whether the explicit semantic limit changed the authorized source span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceContextLimitState {
    NotApplied,
    ExplicitLimitReached,
}

/// One authorized, deterministically bounded source-context observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedSourceContext {
    pub source_context_id: Arc<str>,
    pub identity_recipe: serde_json::Value,
    pub span: SourceSpanIdentity,
    pub authorization_scope: Arc<str>,
    pub content: SourceContextContent,
    pub returned_bytes: usize,
    pub omitted_bytes: usize,
    pub complete: bool,
    pub explicit_source_byte_limit: usize,
    pub limit_state: SourceContextLimitState,
}

/// Fail-closed source-context boundary errors.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SourceContextMaterializationError {
    #[error("source-context identity preimage contains an invalid identity or kind")]
    InvalidIdentityPreimage,
    #[error("source disclosure is not authorized for this request")]
    SourceAccessDenied,
    #[error("source grant workspace does not match the pinned source span")]
    WorkspaceMismatch,
    #[error("source span or authorization range is invalid")]
    InvalidRange,
    #[error("source span is outside the independently authorized byte range")]
    SpanNotAuthorized,
    #[error("declared source byte length does not match the pinned source image")]
    SourceLengthMismatch,
    #[error("declared source digest does not match the pinned source image")]
    SourceDigestMismatch,
    #[error("source span is outside the pinned source image")]
    SpanOutsideSourceImage,
    #[error("explicit source byte limit must be non-zero")]
    ZeroExplicitLimit,
    #[error("hard source output byte limit must be non-zero")]
    ZeroHardLimit,
    #[error("authorized source output exceeds the hard service byte envelope")]
    HardOutputLimitExceeded,
    #[error("source-context canonical identity derivation failed")]
    CanonicalIdentity,
}

/// Complete immutable preimage for one source-context result identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceContextIdentityInput {
    pub workspace_id: Arc<str>,
    pub analysis_context_id: Arc<str>,
    pub snapshot_id: Arc<str>,
    pub entity_id: Arc<str>,
    pub source_file_id: Arc<str>,
    pub source_generation: u64,
    pub source_content_digest: Arc<str>,
    pub delivered_start_byte: u64,
    pub delivered_end_byte: u64,
    pub delivered_content_digest: Arc<str>,
    pub disclosure_scope_id: Arc<str>,
    pub policy_identity: Arc<str>,
    pub context_kind: Arc<str>,
}

/// Issue a source-context identity for the exact bytes authorized and delivered.
///
/// # Errors
///
/// Rejects invalid bounded values or an unrepresentable canonical preimage.
pub fn issue_source_context_identity(
    input: &SourceContextIdentityInput,
) -> Result<CanonicalPublicIdentity, SourceContextMaterializationError> {
    if [
        input.workspace_id.as_ref(),
        input.analysis_context_id.as_ref(),
        input.snapshot_id.as_ref(),
        input.entity_id.as_ref(),
        input.source_file_id.as_ref(),
        input.source_content_digest.as_ref(),
        input.delivered_content_digest.as_ref(),
        input.disclosure_scope_id.as_ref(),
        input.context_kind.as_ref(),
        input.policy_identity.as_ref(),
    ]
    .into_iter()
    .any(|value| value.is_empty() || value.trim() != value || value.len() > 512)
    {
        return Err(SourceContextMaterializationError::InvalidIdentityPreimage);
    }
    if input.delivered_start_byte > input.delivered_end_byte || !input.context_kind.is_ascii() {
        return Err(SourceContextMaterializationError::InvalidIdentityPreimage);
    }
    let workspace_id = decode_public_id(IdentityDomain::Workspace, None, &input.workspace_id)
        .map_err(|_| SourceContextMaterializationError::InvalidIdentityPreimage)?;
    let analysis_context_id = decode_public_id(
        IdentityDomain::AnalysisContext,
        None,
        &input.analysis_context_id,
    )
    .map_err(|_| SourceContextMaterializationError::InvalidIdentityPreimage)?;
    let snapshot_id = decode_public_id(IdentityDomain::ServingSnapshot, None, &input.snapshot_id)
        .map_err(|_| SourceContextMaterializationError::InvalidIdentityPreimage)?;
    let entity_id = decode_public_id_any_kind(IdentityDomain::Entity, &input.entity_id)
        .map_err(|_| SourceContextMaterializationError::InvalidIdentityPreimage)?;
    let file_id = decode_public_id(IdentityDomain::SourceFile, None, &input.source_file_id)
        .map_err(|_| SourceContextMaterializationError::InvalidIdentityPreimage)?;
    let source_digest = decode_b3_digest(&input.source_content_digest)
        .map_err(|_| SourceContextMaterializationError::InvalidIdentityPreimage)?;
    let delivered_digest = decode_b3_digest(&input.delivered_content_digest)
        .map_err(|_| SourceContextMaterializationError::InvalidIdentityPreimage)?;
    let disclosure_scope_id = decode_public_id(
        IdentityDomain::AccessScope,
        None,
        &input.disclosure_scope_id,
    )
    .map_err(|_| SourceContextMaterializationError::InvalidIdentityPreimage)?;
    let context_kind = input.context_kind.to_ascii_lowercase();
    let record = recipes::query_source_context(recipes::QuerySourceContextFields {
        workspace_id: RecipeValue::Id(workspace_id),
        analysis_context_id: RecipeValue::Id(analysis_context_id),
        snapshot_id: RecipeValue::Id(snapshot_id),
        entity_id: RecipeValue::Id(entity_id),
        source_file_id: RecipeValue::Id(file_id),
        source_generation: RecipeValue::Unsigned(input.source_generation.to_be_bytes().to_vec()),
        source_content_digest: RecipeValue::Digest(source_digest),
        delivered_start_byte: RecipeValue::Unsigned(
            input.delivered_start_byte.to_be_bytes().to_vec(),
        ),
        delivered_end_byte: RecipeValue::Unsigned(input.delivered_end_byte.to_be_bytes().to_vec()),
        delivered_content_digest: RecipeValue::Digest(delivered_digest),
        disclosure_scope_id: RecipeValue::Id(disclosure_scope_id),
        policy_identity: RecipeValue::Utf8(input.policy_identity.to_string()),
        context_kind: RecipeValue::Utf8(context_kind.clone()),
    })
    .map_err(|_| SourceContextMaterializationError::CanonicalIdentity)?;
    derive_public_recipe_identity(
        record,
        vec![
            ("workspace_id", serde_json::json!(input.workspace_id)),
            (
                "analysis_context_id",
                serde_json::json!(input.analysis_context_id),
            ),
            ("snapshot_id", serde_json::json!(input.snapshot_id)),
            ("entity_id", serde_json::json!(input.entity_id)),
            ("source_file_id", serde_json::json!(input.source_file_id)),
            (
                "source_generation",
                serde_json::json!(input.source_generation),
            ),
            (
                "source_content_digest",
                serde_json::json!(input.source_content_digest),
            ),
            (
                "delivered_start_byte",
                serde_json::json!(input.delivered_start_byte),
            ),
            (
                "delivered_end_byte",
                serde_json::json!(input.delivered_end_byte),
            ),
            (
                "delivered_content_digest",
                serde_json::json!(input.delivered_content_digest),
            ),
            (
                "disclosure_scope_id",
                serde_json::json!(input.disclosure_scope_id),
            ),
            ("policy_identity", serde_json::json!(input.policy_identity)),
            ("context_kind", serde_json::json!(context_kind)),
        ],
        &["omitted byte count", "truncation state"],
    )
    .map_err(|_| SourceContextMaterializationError::CanonicalIdentity)
}

/// Materialize one exact source span after independent authorization and bounded decoding.
///
/// UTF-8 is returned only when the emitted byte prefix is lossless. An invalid or mid-codepoint
/// prefix remains bytes, allowing the serving layer to encode it without replacement characters.
///
/// # Errors
///
/// Rejects absent source access, mismatched workspaces, invalid/ungranted ranges, source-image
/// drift, zero bounds, or a semantic result that exceeds the separate hard service envelope.
pub fn materialize_authorized_source_context(
    input: SourceContextMaterializationInput<'_>,
) -> Result<MaterializedSourceContext, SourceContextMaterializationError> {
    if !input.grant.source_access {
        return Err(SourceContextMaterializationError::SourceAccessDenied);
    }
    if input.span.workspace_id != input.grant.workspace_id {
        return Err(SourceContextMaterializationError::WorkspaceMismatch);
    }
    if input.span.start_byte > input.span.end_byte
        || input.grant.authorized_start_byte > input.grant.authorized_end_byte
    {
        return Err(SourceContextMaterializationError::InvalidRange);
    }
    if input.span.start_byte < input.grant.authorized_start_byte
        || input.span.end_byte > input.grant.authorized_end_byte
    {
        return Err(SourceContextMaterializationError::SpanNotAuthorized);
    }
    if input.declared_byte_length != input.source_bytes.len() {
        return Err(SourceContextMaterializationError::SourceLengthMismatch);
    }
    if decode_b3_digest(&input.span.content_digest).map_or(true, |expected| {
        expected != *blake3::hash(input.source_bytes).as_bytes()
    }) {
        return Err(SourceContextMaterializationError::SourceDigestMismatch);
    }
    if input.span.end_byte > input.source_bytes.len() {
        return Err(SourceContextMaterializationError::SpanOutsideSourceImage);
    }
    if input.explicit_source_byte_limit == 0 {
        return Err(SourceContextMaterializationError::ZeroExplicitLimit);
    }
    if input.hard_output_byte_limit == 0 {
        return Err(SourceContextMaterializationError::ZeroHardLimit);
    }

    let authorized = &input.source_bytes[input.span.start_byte..input.span.end_byte];
    let returned_bytes = authorized.len().min(input.explicit_source_byte_limit);
    if returned_bytes > input.hard_output_byte_limit {
        return Err(SourceContextMaterializationError::HardOutputLimitExceeded);
    }
    let returned = &authorized[..returned_bytes];
    let delivered_start_byte = u64::try_from(input.span.start_byte)
        .map_err(|_| SourceContextMaterializationError::InvalidRange)?;
    let delivered_end_byte = u64::try_from(input.span.start_byte + returned_bytes)
        .map_err(|_| SourceContextMaterializationError::InvalidRange)?;
    let delivered_content_digest = format!("b3:{}", blake3::hash(returned).to_hex());
    let identity = issue_source_context_identity(&SourceContextIdentityInput {
        workspace_id: Arc::clone(&input.span.workspace_id),
        analysis_context_id: Arc::clone(&input.analysis_context_id),
        snapshot_id: Arc::clone(&input.snapshot_id),
        entity_id: Arc::clone(&input.span.entity_id),
        source_file_id: Arc::clone(&input.span.source_file_id),
        source_generation: input.span.source_generation,
        source_content_digest: Arc::clone(&input.span.content_digest),
        delivered_start_byte,
        delivered_end_byte,
        delivered_content_digest: Arc::from(delivered_content_digest),
        disclosure_scope_id: Arc::clone(&input.grant.authorization_scope),
        policy_identity: Arc::clone(&input.policy_identity),
        context_kind: Arc::clone(&input.context_kind),
    })?;
    let content = match std::str::from_utf8(returned) {
        Ok(text) => SourceContextContent::Text(Arc::from(text)),
        Err(_) => SourceContextContent::Bytes(Arc::from(returned)),
    };
    let omitted_bytes = authorized.len() - returned_bytes;
    let complete = omitted_bytes == 0;
    let identity_recipe = identity.recipe_evidence();
    Ok(MaterializedSourceContext {
        source_context_id: Arc::from(identity.public_id),
        identity_recipe,
        span: input.span,
        authorization_scope: input.grant.authorization_scope,
        content,
        returned_bytes,
        omitted_bytes,
        complete,
        explicit_source_byte_limit: input.explicit_source_byte_limit,
        limit_state: if complete {
            SourceContextLimitState::NotApplied
        } else {
            SourceContextLimitState::ExplicitLimitReached
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_source_context_is_a_delivered_bytes_domain_21_known_answer() {
        let identity = issue_source_context_identity(&SourceContextIdentityInput {
            workspace_id: Arc::from(format!("workspace:{}", "00".repeat(16))),
            analysis_context_id: Arc::from(format!("context:{}", "11".repeat(16))),
            snapshot_id: Arc::from(format!("snapshot:{}", "33".repeat(16))),
            entity_id: Arc::from(format!("entity:function:{}", "44".repeat(16))),
            source_file_id: Arc::from(format!("file:{}", "66".repeat(16))),
            source_generation: 2,
            source_content_digest: Arc::from(format!("b3:{}", "cc".repeat(32))),
            delivered_start_byte: 3,
            delivered_end_byte: 8,
            delivered_content_digest: Arc::from(format!("b3:{}", "dd".repeat(32))),
            disclosure_scope_id: Arc::from(format!("access-scope:{}", "77".repeat(16))),
            policy_identity: Arc::from("policy:r1"),
            context_kind: Arc::from("EXACT_SOURCE_SPAN"),
        })
        .expect("domain-21 source-context KAT");
        assert_eq!(
            identity.public_id,
            "context:fb0ea7d9039e939dc039398e082771be"
        );
        let evidence = identity.recipe_evidence();
        assert_eq!(
            evidence["digest"]["full_digest_hex"],
            "fb0ea7d9039e939dc039398e082771be111f7a9fcbc0dd0579390fe53aaa884c"
        );
        assert_eq!(
            evidence["record_domain"],
            serde_json::json!({"code": 21, "name": "QUERY_SOURCE_CONTEXT"})
        );
        assert_eq!(evidence["fields"][12]["value"], "exact_source_span");
    }

    #[test]
    fn materialization_binds_the_actual_authorized_prefix() {
        let source = b"0123456789";
        let materialized =
            materialize_authorized_source_context(SourceContextMaterializationInput {
                span: SourceSpanIdentity {
                    entity_id: Arc::from(format!("entity:function:{}", "44".repeat(16))),
                    workspace_id: Arc::from(format!("workspace:{}", "00".repeat(16))),
                    source_file_id: Arc::from(format!("file:{}", "66".repeat(16))),
                    content_digest: Arc::from(format!("b3:{}", blake3::hash(source).to_hex())),
                    byte_safe_path: Arc::from("fixture.py"),
                    start_byte: 2,
                    end_byte: 8,
                    source_generation: 2,
                },
                grant: SourceAccessGrant {
                    source_access: true,
                    workspace_id: Arc::from(format!("workspace:{}", "00".repeat(16))),
                    authorized_start_byte: 0,
                    authorized_end_byte: source.len(),
                    authorization_scope: Arc::from(format!("access-scope:{}", "77".repeat(16))),
                },
                analysis_context_id: Arc::from(format!("context:{}", "11".repeat(16))),
                snapshot_id: Arc::from(format!("snapshot:{}", "33".repeat(16))),
                context_kind: Arc::from("exact_source_span"),
                policy_identity: Arc::from("policy:r1"),
                source_bytes: source,
                declared_byte_length: source.len(),
                explicit_source_byte_limit: 3,
                hard_output_byte_limit: 8,
            })
            .expect("authorized bounded source context");
        assert_eq!(materialized.returned_bytes, 3);
        assert_eq!(materialized.omitted_bytes, 3);
        assert_eq!(
            materialized.content,
            SourceContextContent::Text(Arc::from("234"))
        );
        assert_eq!(materialized.identity_recipe["fields"][7]["value"], 2);
        assert_eq!(materialized.identity_recipe["fields"][8]["value"], 5);
        assert_eq!(
            materialized.identity_recipe["fields"][9]["value"],
            format!("b3:{}", blake3::hash(b"234").to_hex())
        );
    }
}
