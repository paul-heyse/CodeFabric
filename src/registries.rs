//! Generated categorical and lifecycle registry types.

include!("generated/registries.rs");

/// Closed failure returned when no generated state-machine edge matches an event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateTransitionViolation {
    pub prior_state: String,
    pub event: String,
    pub guard: String,
    pub error_code: &'static str,
}

/// Resolve one transition solely from a generated transition table.
///
/// # Errors
///
/// Returns `STATE_TRANSITION_VIOLATION` when the state, event, and proven guard do not
/// identify exactly one generated edge.
pub fn generated_transition(
    transitions: &'static [StateTransitionEntry],
    prior_state: &str,
    event: &str,
    guard: &str,
) -> Result<&'static StateTransitionEntry, StateTransitionViolation> {
    let mut matches = transitions.iter().filter(|transition| {
        transition.from == prior_state && transition.event == event && transition.guard == guard
    });
    let transition = matches.next().ok_or_else(|| StateTransitionViolation {
        prior_state: prior_state.to_owned(),
        event: event.to_owned(),
        guard: guard.to_owned(),
        error_code: "STATE_TRANSITION_VIOLATION",
    })?;
    if matches.next().is_some() {
        return Err(StateTransitionViolation {
            prior_state: prior_state.to_owned(),
            event: event.to_owned(),
            guard: guard.to_owned(),
            error_code: "STATE_TRANSITION_VIOLATION",
        });
    }
    Ok(transition)
}

/// Resolve a generated registry code to its canonical state name.
#[must_use]
pub fn registry_state_name(values: &[RegistryEntry], code: u16) -> Option<&'static str> {
    values
        .iter()
        .find(|entry| entry.code == code)
        .map(|entry| entry.name)
}

/// Resolve one generated entity name to its code and family without duplicating either.
#[must_use]
pub fn entity_kind(name: &str) -> Option<OntologyCodeEntry> {
    ontology_kind(ENTITY_KIND_IDS, ENTITY_KIND_CODES, name)
}

/// Resolve one generated relation name to its code and family without duplicating either.
#[must_use]
pub fn relation_kind(name: &str) -> Option<OntologyCodeEntry> {
    ontology_kind(RELATION_KIND_IDS, RELATION_KIND_CODES, name)
}

/// Resolve one generated property name to its code and family without duplicating either.
#[must_use]
pub fn property_kind(name: &str) -> Option<OntologyCodeEntry> {
    ontology_kind(PROPERTY_KIND_IDS, PROPERTY_KIND_CODES, name)
}

/// Resolve a capability identifier to its append-only declaration-order code.
///
/// Capability registry entries are ordered authority records. AC-G-06 assigns registry codes in
/// declaration order starting at 10 and advancing by 10, so consumers never duplicate a second
/// capability allocation table.
#[must_use]
pub fn capability_code(name: &str) -> Option<u16> {
    CAPABILITY_IDS
        .iter()
        .zip(CAPABILITY_CODES)
        .find_map(|(candidate, code)| (*candidate == name).then_some(*code))
}

/// Build the compact owner capability summary from the same generated registry ordering.
#[must_use]
pub fn capability_mask(names: &[&str]) -> Option<u64> {
    names.iter().try_fold(0_u64, |word, name| {
        CAPABILITY_IDS
            .iter()
            .position(|candidate| candidate == name)
            .and_then(|bit| u32::try_from(bit).ok())
            .and_then(|bit| 1_u64.checked_shl(bit))
            .map(|mask| word | mask)
    })
}

fn ontology_kind(
    names: &'static [&'static str],
    codes: &'static [OntologyCodeEntry],
    name: &str,
) -> Option<OntologyCodeEntry> {
    debug_assert_eq!(names.len(), codes.len());
    names
        .iter()
        .position(|candidate| *candidate == name)
        .and_then(|index| codes.get(index).copied())
}

#[cfg(all(test, feature = "canonical-json"))]
mod tests {
    use std::any::TypeId;

    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Vectors {
        enum_triples: Vec<EnumTriple>,
        flag_words: Vec<FlagWord>,
    }

    #[derive(Deserialize)]
    struct EnumTriple {
        domain: String,
        code: u16,
        name: String,
        slug: String,
    }

    #[derive(Deserialize)]
    struct FlagWord {
        domain: String,
        names: Vec<String>,
        word: u64,
    }

    fn values(domain: &str) -> &'static [RegistryEntry] {
        match domain {
            "EVIDENCE_CERTAINTY" => EVIDENCE_CERTAINTY_VALUES,
            "RESOLUTION_CLASS" => RESOLUTION_CLASS_VALUES,
            "DIRECTNESS" => DIRECTNESS_VALUES,
            "COMPLETENESS" => COMPLETENESS_VALUES,
            "OWNER_CAPABILITY_STATE" => OWNER_CAPABILITY_STATE_VALUES,
            "PROVIDER_RUN_STATE" => PROVIDER_RUN_STATE_VALUES,
            "QUERY_EXECUTION_STATE" => QUERY_EXECUTION_STATE_VALUES,
            "QUERY_AVAILABILITY_STATE" => QUERY_AVAILABILITY_STATE_VALUES,
            "COMPLETENESS_STATE" => COMPLETENESS_STATE_VALUES,
            "FRESHNESS_STATE" => FRESHNESS_STATE_VALUES,
            "LIMIT_STATE" => LIMIT_STATE_VALUES,
            "DEPENDENCY_STATE" => DEPENDENCY_STATE_VALUES,
            "DURABLE_PUBLICATION_STATE" => DURABLE_PUBLICATION_STATE_VALUES,
            "SERVING_ACTIVATION_STATE" => SERVING_ACTIVATION_STATE_VALUES,
            "SOURCE_TRUST_STATE" => SOURCE_TRUST_STATE_VALUES,
            "EVENT_STREAM_HEALTH" => EVENT_STREAM_HEALTH_VALUES,
            "GIT_ACCELERATION_STATUS" => GIT_ACCELERATION_STATUS_VALUES,
            "EFFECT_KIND" => EFFECT_KIND_VALUES,
            "RESOURCE_KIND" => RESOURCE_KIND_VALUES,
            other => panic!("unknown KAT domain {other}"),
        }
    }

    #[test]
    fn wp08_behavioral_acceptance() {
        let vectors: Vectors = serde_json::from_str(include_str!(
            "../contracts/fixtures/registries/enum-flag-v1-vectors.json"
        ))
        .unwrap();
        for vector in vectors.enum_triples {
            let entry = values(&vector.domain)
                .iter()
                .find(|entry| entry.code == vector.code)
                .unwrap();
            assert_eq!(
                (entry.name, entry.slug),
                (vector.name.as_str(), vector.slug.as_str())
            );
        }
        for vector in vectors.flag_words {
            assert_eq!(vector.domain, "FACT_FLAGS");
            let computed = FACT_FLAGS_FLAGS
                .iter()
                .filter(|entry| vector.names.iter().any(|name| name == entry.name))
                .fold(0_u64, |word, entry| word | entry.mask);
            assert_eq!(computed, vector.word);
        }
    }

    #[test]
    fn wp08_structural_acceptance() {
        assert_ne!(
            TypeId::of::<EvidenceCertainty>(),
            TypeId::of::<ResolutionClass>()
        );
        assert_ne!(
            TypeId::of::<Completeness>(),
            TypeId::of::<CompletenessState>()
        );
        assert_eq!(EFFECT_KIND_VALUES.len(), 37);
        assert_eq!(RESOURCE_KIND_VALUES.len(), 10);
        assert_eq!(PROJECTION_IDS.len(), 13);
        assert_eq!(CAPABILITY_IDS.len(), 22);
        assert_eq!(PUBLIC_ERROR_IDS.len(), 61);
    }

    #[test]
    fn wp08b_behavioral_acceptance() {
        assert_eq!(PHRASE_ENTRIES.len(), 45);
        let call_targets = PHRASE_ENTRIES
            .iter()
            .find(|entry| entry.phrase_id == "Q57_CALL_TARGETS")
            .unwrap();
        assert_eq!(call_targets.owner_section, 57);
        assert_eq!(call_targets.canonical_text, "call targets");
        assert_eq!(call_targets.accepted_aliases, &["dispatch targets"]);
        assert_eq!(call_targets.plan_node_kind, "follow-relationships");
        assert_eq!(call_targets.output_role, "fact-set");
    }

    #[test]
    fn wp08b_operational_acceptance() {
        assert_eq!(PHRASE_IDS.len(), PHRASE_ENTRIES.len());
        assert_eq!(PHRASE_IDS.first(), Some(&"Q50_SOURCE_FILES"));
        assert_eq!(PHRASE_IDS.last(), Some(&"Q94_RUST_COMPILE_TIME_VALUES"));
        assert!(
            PHRASE_ENTRIES
                .windows(2)
                .all(|entries| { entries[0].owner_section + 1 == entries[1].owner_section })
        );
    }

    #[test]
    fn wp56_behavioral_acceptance() {
        assert_eq!(
            crate::identity::IdentityDomain::RootAuthorization as u16,
            17
        );
        assert_eq!(
            QUERY_FORM_VALUES
                .iter()
                .map(|entry| entry.slug)
                .collect::<Vec<_>>(),
            vec![
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
        assert_eq!(NewlineKind::Crlf as u16, 30);
        assert_eq!(FreshnessState::PotentiallyStale as u16, 20);
        assert_eq!(capability_code("RUST_MIR"), Some(120));
    }

    #[test]
    fn wp56_structural_acceptance() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(!root.join("src/generated/model_registries.rs").exists());
        assert!(
            !root
                .join("codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/registries.py")
                .exists()
        );
        assert_eq!(
            include_bytes!("generated/digest_frames.rs"),
            include_bytes!("../rustc-extractor/src/generated/digest_frames.rs")
        );
        assert_eq!(CAPABILITY_IDS.len(), CAPABILITY_CODES.len());
    }
}
