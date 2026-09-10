//! Exact provider messages, with compiler-unit ownership separate from diagnostic locations.

use std::sync::Arc;

mod details;
pub(super) use details::Detail;

use crate::identity::{CbefField, CbefRecord, CbefValue, IdentityDomain, StringNormalization};
use crate::pyrefly_service::PyreflyRelation;

use super::{
    DataFusionError, DataType, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, RUN,
    RustcRelation, SOURCE, ScalarUDF, ScalarValue, TransformationInputs, TransformationPlanError,
    Volatility, col, create_udf, empty, file_id_udf, fixed, ids, invalid, lit, number, plan,
    source_alias, text,
};

pub(super) const RELATION: &str = "fact.code_diagnostic";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Inputs {
    pub primary: bool,
    pub details: [bool; 4],
}

impl Inputs {
    pub fn from_relations(relations: &std::collections::BTreeSet<RustcRelation>) -> Self {
        let primary = relations.contains(&RustcRelation::Diagnostic);
        Self {
            primary,
            details: Detail::ALL.map(|detail| primary && relations.contains(&detail.raw())),
        }
    }
}

pub(super) fn fields() -> Vec<FieldSpec> {
    vec![
        ("diagnostic_id", DataType::FixedSizeBinary(16), false),
        ("language", DataType::Utf8, false),
        ("context_id", DataType::FixedSizeBinary(16), false),
        ("workspace_id", DataType::FixedSizeBinary(16), false),
        ("source_generation", DataType::UInt64, false),
        ("owner_file_id", DataType::FixedSizeBinary(16), true),
        ("owner_content_digest", DataType::FixedSizeBinary(32), false),
        ("provider", DataType::Utf8, false),
        ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ("provider_unit_id", DataType::Utf8, false),
        ("diagnostic_ordinal", DataType::UInt64, false),
        ("severity", DataType::Utf8, true),
        ("code", DataType::Utf8, true),
        ("message", DataType::Utf8, false),
        ("structured_fields_available", DataType::Boolean, false),
        ("suggestions_state", DataType::Utf8, true),
        ("authority", DataType::Utf8, false),
    ]
}

pub(super) fn dependencies(pyrefly: bool, rust: bool) -> Vec<&'static str> {
    if !pyrefly && !rust {
        return vec![];
    }
    let mut result = vec![SOURCE, RUN];
    if pyrefly {
        result.push(PyreflyRelation::Diagnostic.relation_id());
    }
    if rust {
        result.push(RustcRelation::Diagnostic.relation_id());
    }
    result
}

pub(super) fn build(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    pyrefly: bool,
    rust: bool,
) -> Result<LogicalPlan, TransformationPlanError> {
    let mut plans = Vec::new();
    for (available, provider) in [(pyrefly, Provider::Python), (rust, Provider::Rust)] {
        if available {
            plans.push(provider_plan(workspace, inputs, provider)?);
        }
    }
    let Some(first) = plans.pop() else {
        return empty(fields());
    };
    let mut result = LogicalPlanBuilder::from(first);
    for plan in plans {
        result = result.union(plan)?;
    }
    Ok(result.build()?)
}

#[derive(Clone, Copy)]
enum Provider {
    Python,
    Rust,
}

impl Provider {
    fn name(self) -> &'static str {
        match self {
            Self::Python => "pyrefly",
            Self::Rust => "rustc",
        }
    }
    fn language(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Rust => "rust",
        }
    }
    fn relation(self) -> &'static str {
        match self {
            Self::Python => PyreflyRelation::Diagnostic.relation_id(),
            Self::Rust => RustcRelation::Diagnostic.relation_id(),
        }
    }
    fn source(self) -> &'static str {
        match self {
            Self::Python => "p.file_id",
            Self::Rust => "p.source_file_id",
        }
    }
    fn digest(self) -> &'static str {
        match self {
            Self::Python => "p.content_digest",
            Self::Rust => "p.source_content_digest",
        }
    }
    fn unit(self) -> &'static str {
        match self {
            Self::Python => "p.module_id",
            Self::Rust => "p.compilation_unit_id",
        }
    }
    fn message(self) -> &'static str {
        match self {
            Self::Python => "p.rendered_text",
            Self::Rust => "p.message",
        }
    }
}

fn provider_plan(
    workspace: [u8; 16],
    inputs: &TransformationInputs,
    provider: Provider,
) -> Result<LogicalPlan, TransformationPlanError> {
    let raw = LogicalPlanBuilder::from(plan(inputs, provider.relation())?)
        .alias("p")?
        .build()?;
    let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
        .filter(col("provider").eq(lit(provider.name())))?
        .alias("r")?
        .build()?;
    let owner = file_id_udf().call(vec![col(provider.source())]);
    let joined = LogicalPlanBuilder::from(raw)
        .join(
            runs,
            JoinType::Inner,
            (
                vec!["p.provider_run_id", "p.source_generation"],
                vec!["r.provider_run_identity", "r.source_generation"],
            ),
            None,
        )?
        .join_on(
            source_alias(inputs)?,
            JoinType::Inner,
            vec![
                owner.clone().eq(col("s.file_id")),
                col(provider.digest()).eq(col("s.content_digest")),
                col("p.source_generation").eq(col("s.source_generation")),
            ],
        )?;
    let (severity, code, structured, suggestions, authority) = match provider {
        Provider::Python => (
            lit(ScalarValue::Utf8(None)),
            lit(ScalarValue::Utf8(None)),
            col("p.structured_fields_available"),
            lit(ScalarValue::Utf8(None)),
            "pyrefly-rendered-diagnostic",
        ),
        Provider::Rust => (
            col("p.severity"),
            col("p.reason_code"),
            col("p.structured_compiler_diagnostic"),
            col("p.suggestions_state"),
            "rustc-structured-diagnostic",
        ),
    };
    let id = diagnostic_id(workspace).call(vec![
        col("r.context_id"),
        owner.clone(),
        col(provider.digest()),
        lit(provider.name()),
        col(provider.unit()),
        col("p.diagnostic_ordinal"),
        col(provider.message()),
    ]);
    Ok(joined
        .project(vec![
            id.alias("diagnostic_id"),
            lit(provider.language()).alias("language"),
            col("r.context_id").alias("context_id"),
            col("s.workspace_id").alias("workspace_id"),
            col("p.source_generation").alias("source_generation"),
            owner.alias("owner_file_id"),
            col(provider.digest()).alias("owner_content_digest"),
            lit(provider.name()).alias("provider"),
            col("r.provider_run_id").alias("provider_run_id"),
            col(provider.unit()).alias("provider_unit_id"),
            col("p.diagnostic_ordinal").alias("diagnostic_ordinal"),
            severity.alias("severity"),
            code.alias("code"),
            col(provider.message()).alias("message"),
            structured.alias("structured_fields_available"),
            suggestions.alias("suggestions_state"),
            lit(authority).alias("authority"),
        ])?
        .build()?)
}

fn diagnostic_id(workspace: [u8; 16]) -> Arc<ScalarUDF> {
    Arc::new(create_udf(
        "codefabric_canonical_diagnostic_id_v1",
        vec![
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(16),
            DataType::FixedSizeBinary(32),
            DataType::Utf8,
            DataType::Utf8,
            DataType::UInt64,
            DataType::Utf8,
        ],
        DataType::FixedSizeBinary(16),
        Volatility::Immutable,
        Arc::new(move |values| {
            ids(values, |arrays, row| {
                let digest: [u8; 32] = arrays[2]
                    .as_any()
                    .downcast_ref::<super::FixedSizeBinaryArray>()
                    .ok_or_else(|| invalid("expected diagnostic content digest"))?
                    .value(row)
                    .try_into()
                    .map_err(|_| invalid("invalid diagnostic content digest width"))?;
                let utf8 = |value: &str| CbefValue::Utf8 {
                    value: value.to_owned(),
                    normalization: StringNormalization::None,
                };
                let values = [
                    utf8("codefabric.canonical-diagnostic.v1"),
                    CbefValue::Id(workspace),
                    CbefValue::Id(fixed(&arrays[0], row)?),
                    CbefValue::Id(fixed(&arrays[1], row)?),
                    CbefValue::Digest(digest),
                    utf8(text(&arrays[3], row)?),
                    utf8(text(&arrays[4], row)?),
                    CbefValue::Unsigned(number(&arrays[5], row)?.to_be_bytes().to_vec()),
                    utf8(text(&arrays[6], row)?),
                ];
                crate::identity::derive_identity(&CbefRecord {
                    domain: IdentityDomain::RelationFact,
                    fields: values
                        .into_iter()
                        .zip(1_u16..)
                        .map(|(value, tag)| CbefField { tag, value })
                        .collect(),
                })
                .map(|id| id.id)
                .map_err(|error| DataFusionError::Execution(error.to_string()))
            })
        }),
    ))
}
