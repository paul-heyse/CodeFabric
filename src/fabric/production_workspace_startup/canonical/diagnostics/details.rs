//! Native diagnostic hierarchy and exact, separately fenced source locations.

use super::super::{
    DataType, Expr, FieldSpec, JoinType, LogicalPlan, LogicalPlanBuilder, RUN, RustcRelation,
    SOURCE, ScalarValue, TransformationInputs, TransformationPlanError, col, empty, file_id_udf,
    lit, plan, source_alias,
};
use datafusion::logical_expr::when;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Detail {
    Child,
    Span,
    Suggestion,
    Edit,
}

impl Detail {
    pub const ALL: [Self; 4] = [Self::Child, Self::Span, Self::Suggestion, Self::Edit];

    pub fn raw(self) -> RustcRelation {
        match self {
            Self::Child => RustcRelation::DiagnosticChild,
            Self::Span => RustcRelation::DiagnosticSpan,
            Self::Suggestion => RustcRelation::DiagnosticSuggestion,
            Self::Edit => RustcRelation::DiagnosticEdit,
        }
    }

    pub fn relation(self) -> &'static str {
        match self {
            Self::Child => "fact.code_diagnostic_child",
            Self::Span => "fact.code_diagnostic_span",
            Self::Suggestion => "fact.code_diagnostic_suggestion",
            Self::Edit => "fact.code_diagnostic_edit",
        }
    }

    fn has_location(self) -> bool {
        matches!(self, Self::Span | Self::Edit)
    }

    fn native_fields(self) -> Vec<FieldSpec> {
        let ordinal = |name, nullable| (name, DataType::UInt64, nullable);
        let text = |name, nullable| (name, DataType::Utf8, nullable);
        match self {
            Self::Child => vec![
                ordinal("child_ordinal", false),
                text("severity", false),
                text("message", false),
            ],
            Self::Span => vec![
                ordinal("child_ordinal", true),
                ordinal("span_ordinal", false),
                ("is_primary", DataType::Boolean, false),
                text("label", true),
            ],
            Self::Suggestion => vec![
                ordinal("suggestion_ordinal", false),
                text("message", false),
                text("style", false),
                text("applicability", false),
                ordinal("alternative_count", false),
                ordinal("alternative_ordinal", true),
                ordinal("part_count", false),
            ],
            Self::Edit => vec![
                ordinal("suggestion_ordinal", false),
                ordinal("alternative_ordinal", false),
                ordinal("part_ordinal", false),
                text("replacement_text", false),
            ],
        }
    }

    pub fn fields(self) -> Vec<FieldSpec> {
        let mut fields = vec![
            ("diagnostic_id", DataType::FixedSizeBinary(16), false),
            ("language", DataType::Utf8, false),
            ("context_id", DataType::FixedSizeBinary(16), false),
            ("workspace_id", DataType::FixedSizeBinary(16), false),
            ("source_generation", DataType::UInt64, false),
            ("provider_run_id", DataType::FixedSizeBinary(16), false),
        ];
        fields.extend(self.native_fields());
        if self.has_location() {
            fields.extend([
                ("file_id", DataType::FixedSizeBinary(16), true),
                ("content_digest", DataType::FixedSizeBinary(32), true),
                ("start_byte", DataType::UInt64, true),
                ("end_byte", DataType::UInt64, true),
                ("location_state", DataType::Utf8, false),
                ("expansion_kind", DataType::Utf8, false),
                ("native_file", DataType::Utf8, true),
                ("native_file_bytes", DataType::Binary, true),
                ("native_start_byte", DataType::UInt64, true),
                ("native_end_byte", DataType::UInt64, true),
            ]);
        }
        fields
    }

    pub fn dependencies(self, available: bool) -> Vec<&'static str> {
        if available {
            let mut dependencies = vec![super::RELATION, RUN, self.raw().relation_id()];
            if self.has_location() {
                dependencies.push(SOURCE);
            }
            dependencies
        } else {
            vec![]
        }
    }

    pub fn build(
        self,
        inputs: &TransformationInputs,
        available: bool,
    ) -> Result<LogicalPlan, TransformationPlanError> {
        if !available {
            return empty(self.fields());
        }
        let raw = LogicalPlanBuilder::from(plan(inputs, self.raw().relation_id())?)
            .alias("p")?
            .build()?;
        let runs = LogicalPlanBuilder::from(plan(inputs, RUN)?)
            .filter(col("provider").eq(lit("rustc")))?
            .alias("r")?
            .build()?;
        let messages = LogicalPlanBuilder::from(plan(inputs, super::RELATION)?)
            .filter(col("provider").eq(lit("rustc")))?
            .alias("d")?
            .build()?;
        let mut joined = LogicalPlanBuilder::from(raw)
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
                messages,
                JoinType::Inner,
                vec![
                    col("r.provider_run_id").eq(col("d.provider_run_id")),
                    col("p.source_generation").eq(col("d.source_generation")),
                    col("p.compilation_unit_id").eq(col("d.provider_unit_id")),
                    col("p.diagnostic_ordinal").eq(col("d.diagnostic_ordinal")),
                    file_id_udf()
                        .call(vec![col("p.source_file_id")])
                        .eq(col("d.owner_file_id")),
                    col("p.source_content_digest").eq(col("d.owner_content_digest")),
                ],
            )?;
        let mut projection = [
            "diagnostic_id",
            "language",
            "context_id",
            "workspace_id",
            "source_generation",
            "provider_run_id",
        ]
        .into_iter()
        .map(|name| col(format!("d.{name}")).alias(name))
        .collect::<Vec<_>>();
        projection.extend(
            self.native_fields()
                .iter()
                .map(|(name, _, _)| col(format!("p.{name}")).alias(*name)),
        );
        if self.has_location() {
            joined = joined.join_on(
                source_alias(inputs)?,
                JoinType::Left,
                vec![
                    file_id_udf()
                        .call(vec![col("p.location_file_id")])
                        .eq(col("s.file_id")),
                    col("p.location_content_digest").eq(col("s.content_digest")),
                    col("p.source_generation").eq(col("s.source_generation")),
                    col("p.span_start_byte").lt_eq(col("p.span_end_byte")),
                    col("p.span_end_byte").lt_eq(col("s.byte_length")),
                ],
            )?;
            projection.extend(location_projection()?);
        }
        Ok(joined.project(projection)?.build()?)
    }
}

fn location_projection() -> Result<Vec<Expr>, datafusion::common::DataFusionError> {
    let bound = col("s.file_id").is_not_null();
    let start =
        when(bound.clone(), col("p.span_start_byte")).otherwise(lit(ScalarValue::UInt64(None)))?;
    let end =
        when(bound.clone(), col("p.span_end_byte")).otherwise(lit(ScalarValue::UInt64(None)))?;
    let state = when(bound, lit("captured-source"))
        .when(
            col("p.location_file_id").is_not_null(),
            lit("invalidated-source-location"),
        )
        .otherwise(col("p.location_state"))?;
    Ok(vec![
        col("s.file_id").alias("file_id"),
        col("s.content_digest").alias("content_digest"),
        start.alias("start_byte"),
        end.alias("end_byte"),
        state.alias("location_state"),
        col("p.expansion_kind").alias("expansion_kind"),
        col("p.span_file").alias("native_file"),
        col("p.span_file_bytes").alias("native_file_bytes"),
        col("p.span_start_byte").alias("native_start_byte"),
        col("p.span_end_byte").alias("native_end_byte"),
    ])
}
