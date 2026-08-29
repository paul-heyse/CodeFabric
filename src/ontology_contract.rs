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

/// Derive the canonical content identity for one authored rule operation and its operands.
///
/// The encoding is explicitly length-framed so concatenation cannot create an ambiguous
/// preimage. Operand order remains semantic and is therefore included verbatim.
pub fn rule_semantics_identity<'a>(
    operation_kind: &str,
    operands: impl IntoIterator<Item = (u16, &'a str, &'a str, &'a str)>,
) -> String {
    let mut material = Vec::new();
    let append = |material: &mut Vec<u8>, part: &[u8]| {
        material.extend_from_slice(
            &u64::try_from(part.len())
                .expect("rule semantics component length")
                .to_be_bytes(),
        );
        material.extend_from_slice(part);
    };
    append(&mut material, operation_kind.as_bytes());
    for (ordinal, relation_ref, column_ref, logical_type) in operands {
        append(&mut material, &ordinal.to_be_bytes());
        append(&mut material, relation_ref.as_bytes());
        append(&mut material, column_ref.as_bytes());
        append(&mut material, logical_type.as_bytes());
    }
    format!("b3:{}", blake3::hash(&material).to_hex())
}

#[cfg(test)]
mod tests {
    use super::rule_semantics_identity;

    #[test]
    fn rule_identity_is_ordered_and_unambiguous() {
        let left = rule_semantics_identity(
            "JOIN",
            [(0, "ab", "c", "relation"), (1, "d", "e", "column")],
        );
        let reordered = rule_semantics_identity(
            "JOIN",
            [(1, "d", "e", "column"), (0, "ab", "c", "relation")],
        );
        let regrouped = rule_semantics_identity(
            "JOIN",
            [(0, "a", "bc", "relation"), (1, "d", "e", "column")],
        );
        assert_ne!(left, reordered);
        assert_ne!(left, regrouped);
    }
}
