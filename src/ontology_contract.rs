//! Shared ontology-program contract identity primitives.

/// Pinned DataFusion 55 expression census shared by model compilation and runtime policy.
pub const DATAFUSION_EXPR_VARIANT_CENSUS: &[&str] = &[
    "Alias",
    "Column",
    "ScalarVariable",
    "Literal",
    "BinaryExpr",
    "Like",
    "SimilarTo",
    "Not",
    "IsNotNull",
    "IsNull",
    "IsTrue",
    "IsFalse",
    "IsUnknown",
    "IsNotTrue",
    "IsNotFalse",
    "IsNotUnknown",
    "Negative",
    "Between",
    "Case",
    "Cast",
    "TryCast",
    "ScalarFunction",
    "AggregateFunction",
    "WindowFunction",
    "InList",
    "Exists",
    "InSubquery",
    "SetComparison",
    "ScalarSubquery",
    "Wildcard",
    "GroupingSet",
    "Placeholder",
    "OuterReferenceColumn",
    "Unnest",
    "HigherOrderFunction",
    "Lambda",
    "LambdaVariable",
];

/// Derive one unambiguous record identity for an authored ontology graph row.
///
/// The encoding is explicitly length-framed so concatenation cannot create an ambiguous
/// preimage. Operand order remains semantic and is therefore included verbatim.
///
/// # Panics
///
/// Panics only if a component length cannot be represented as `u64`, which is impossible on the
/// supported 64-bit targets.
pub fn ontology_semantics_record<S>(
    record_kind: &str,
    fields: impl IntoIterator<Item = S>,
) -> String
where
    S: AsRef<str>,
{
    let mut material = Vec::new();
    let append = |material: &mut Vec<u8>, part: &[u8]| {
        material.extend_from_slice(
            &u64::try_from(part.len())
                .expect("rule semantics component length")
                .to_be_bytes(),
        );
        material.extend_from_slice(part);
    };
    append(&mut material, record_kind.as_bytes());
    for field in fields {
        append(&mut material, field.as_ref().as_bytes());
    }
    format!("b3:{}", blake3::hash(&material).to_hex())
}

/// Derive the canonical content identity of one rule's complete authored graph.
///
/// Record identities are sorted because plan and operand ordinals carry semantic order. Source
/// array ordering is deliberately non-semantic, so formatting-only row movement cannot alter the
/// compiled program identity.
///
/// # Panics
///
/// Panics only when one record length cannot be represented by `u64`, which cannot occur on the
/// supported 64-bit targets.
pub fn rule_semantics_identity<'a>(records: impl IntoIterator<Item = &'a str>) -> String {
    let mut records = records.into_iter().collect::<Vec<_>>();
    records.sort_unstable();
    let mut material = Vec::new();
    for record in records {
        material.extend_from_slice(
            &u64::try_from(record.len())
                .expect("rule semantics record length")
                .to_be_bytes(),
        );
        material.extend_from_slice(record.as_bytes());
    }
    format!("b3:{}", blake3::hash(&material).to_hex())
}

#[cfg(test)]
mod tests {
    use super::{ontology_semantics_record, rule_semantics_identity};

    #[test]
    fn rule_identity_is_ordered_and_unambiguous() {
        let left_record = ontology_semantics_record("JOIN", ["ab", "c", "relation"]);
        let regrouped_record = ontology_semantics_record("JOIN", ["a", "bc", "relation"]);
        let left = rule_semantics_identity([left_record.as_str(), "b3:second"]);
        let reordered = rule_semantics_identity(["b3:second", left_record.as_str()]);
        let regrouped = rule_semantics_identity([regrouped_record.as_str(), "b3:second"]);
        assert_eq!(left, reordered);
        assert_ne!(left, regrouped);
    }
}
