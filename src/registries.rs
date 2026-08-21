//! Generated categorical and lifecycle registry types.

include!("generated/registries.rs");

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
        assert_eq!(PUBLIC_ERROR_IDS.len(), 60);
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
}
