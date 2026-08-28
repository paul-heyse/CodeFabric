//! Closed typed ontology-rule contracts and their DataFusion lowering.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{Int16Array, Int32Array, RecordBatch};
use arrow_schema::Schema;
use datafusion::datasource::MemTable;
use datafusion::functions_aggregate::expr_fn::count;
use datafusion::logical_expr::expr_fn::cast;
use datafusion::logical_expr::{Expr, JoinType, col, lit};
use datafusion::prelude::{DataFrame, SessionContext};

use crate::compiled_ontology::{CompiledRuleContract, compiled_ontology};
use crate::fabric::FabricError;
use crate::schema_registry::{SemanticAuthority, semantic_type_binding, table_spec};

#[must_use]
pub fn rule_contracts() -> &'static [CompiledRuleContract] {
    compiled_ontology().rules
}

fn frame(batch: &RecordBatch) -> Result<DataFrame, FabricError> {
    let context = SessionContext::new();
    let validation_schema = Arc::new(Schema::new(batch.schema().fields().clone()));
    let validation_batch =
        RecordBatch::try_new(validation_schema.clone(), batch.columns().to_vec())?;
    Ok(context.read_table(Arc::new(MemTable::try_new(
        validation_schema,
        vec![vec![validation_batch]],
    )?))?)
}

async fn rejects_any(frame: DataFrame) -> Result<bool, FabricError> {
    Ok(frame
        .limit(0, Some(1))?
        .collect()
        .await?
        .first()
        .is_some_and(|batch| batch.num_rows() != 0))
}

async fn validate_primary_keys(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    for (&table_code, batch) in batches {
        let spec = table_spec(table_code).expect("candidate table is generated");
        if spec.primary_key.is_empty() || batch.num_rows() < 2 {
            continue;
        }
        let keys = spec
            .primary_key
            .iter()
            .map(|name| col(*name))
            .collect::<Vec<_>>();
        let invalid = frame(batch)?
            .aggregate(keys, vec![count(lit(1_i64)).alias("row_count")])?
            .filter(col("row_count").gt(lit(1_i64)))?;
        if rejects_any(invalid).await? {
            return Err(FabricError::PublicationIntegrity(format!(
                "{} violates compiled rule ontology.primary-key.v1",
                spec.name
            )));
        }
    }
    Ok(())
}

fn code_dimension(semantic_type: &str) -> Option<(i16, &'static str, Option<&'static str>)> {
    let binding = semantic_type_binding(semantic_type)?;
    match binding.authority {
        SemanticAuthority::EnumRegistry => Some((11, "code", binding.domain)),
        SemanticAuthority::OntologyEntityRegistry => match binding.domain {
            Some("ENTITY_FAMILY") => Some((13, "code", None)),
            _ => Some((12, "code", None)),
        },
        SemanticAuthority::OntologyRelationRegistry => match binding.domain {
            Some("RELATION_FAMILY") => Some((15, "code", None)),
            _ => Some((14, "code", None)),
        },
        SemanticAuthority::OntologyPropertyRegistry => Some((16, "code", None)),
        SemanticAuthority::OntologyFactRegistry => Some((17, "code", None)),
        _ => None,
    }
}

async fn validate_governed_codes(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    for (&table_code, source) in batches {
        let spec = table_spec(table_code).expect("candidate table is generated");
        for field in source.schema().fields() {
            let Some(semantic_type) = field.metadata().get("com.codefabric.cpg.semantic_type")
            else {
                continue;
            };
            let Some((dimension_code, dimension_column, domain)) = code_dimension(semantic_type)
            else {
                continue;
            };
            let Some(dimension) = batches.get(&dimension_code) else {
                return Err(FabricError::PublicationIntegrity(format!(
                    "compiled governed-code rule lacks dimension {dimension_code}"
                )));
            };
            let mut target = frame(dimension)?;
            if let Some(domain) = domain {
                target = target.filter(col("domain").eq(lit(domain)))?;
            }
            target = target.select(vec![
                cast(col(dimension_column), arrow_schema::DataType::Int64).alias("governed_code"),
            ])?;
            let source_frame = frame(source)?
                .filter(col(field.name()).is_not_null())?
                .select(vec![
                    cast(col(field.name()), arrow_schema::DataType::Int64).alias("candidate_code"),
                ])?;
            let invalid = source_frame.join(
                target,
                JoinType::LeftAnti,
                &["candidate_code"],
                &["governed_code"],
                None,
            )?;
            if rejects_any(invalid).await? {
                return Err(FabricError::PublicationIntegrity(format!(
                    "{}.{} violates compiled governed-code rule",
                    spec.name,
                    field.name()
                )));
            }
        }
    }
    Ok(())
}

async fn validate_property_one_of(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    let Some(properties) = batches.get(&120) else {
        return Ok(());
    };
    let names = [
        "value_entity_id",
        "value_bool",
        "value_int64",
        "value_float64",
        "value_text",
        "value_bytes",
        "value_type_id",
    ];
    let none = names
        .iter()
        .map(|name| col(*name).is_null())
        .reduce(Expr::and)
        .expect("nonempty value set");
    let mut multiple = None;
    for (index, left) in names.iter().enumerate() {
        for right in &names[index + 1..] {
            let pair = col(*left).is_not_null().and(col(*right).is_not_null());
            multiple = Some(multiple.map_or(pair.clone(), |prior: Expr| prior.or(pair)));
        }
    }
    let kind = col("value_kind_code");
    let tag_mismatch = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let code = i16::try_from((index + 1) * 10).expect("seven value kinds");
            kind.clone()
                .eq(lit(code))
                .and(col(*name).is_null())
                .or(kind.clone().not_eq(lit(code)).and(col(*name).is_not_null()))
        })
        .reduce(Expr::or)
        .expect("nonempty value tags");
    let invalid =
        frame(properties)?.filter(none.or(multiple.expect("multiple pairs")).or(tag_mismatch))?;
    if rejects_any(invalid).await? {
        return Err(FabricError::PublicationIntegrity(
            "property_fact violates compiled one-of rule".into(),
        ));
    }
    Ok(())
}

async fn validate_membership_edges(
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<(), FabricError> {
    let (Some(edges), Some(terms)) = (batches.get(&21), batches.get(&20)) else {
        return Ok(());
    };
    let targets = frame(terms)?.select(vec![col("term_id").alias("known_term_id")])?;
    for endpoint in ["subject_term_id", "predicate_term_id", "object_term_id"] {
        let source = frame(edges)?.select(vec![col(endpoint).alias("candidate_term_id")])?;
        let invalid = source.join(
            targets.clone(),
            JoinType::LeftAnti,
            &["candidate_term_id"],
            &["known_term_id"],
            None,
        )?;
        if rejects_any(invalid).await? {
            return Err(FabricError::PublicationIntegrity(format!(
                "ontology_edge.{endpoint} violates compiled membership closure"
            )));
        }
    }
    Ok(())
}

async fn validate_relation_family(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    let (Some(relations), Some(kinds)) = (batches.get(&110), batches.get(&14)) else {
        return Ok(());
    };
    let facts = frame(relations)?.select(vec![
        col("relation_kind_code").alias("fact_kind"),
        col("relation_family_code").alias("fact_family"),
    ])?;
    let dimensions = frame(kinds)?.select(vec![
        col("code").alias("dimension_kind"),
        col("family_code").alias("dimension_family"),
    ])?;
    let invalid = facts
        .join(
            dimensions,
            JoinType::Inner,
            &["fact_kind"],
            &["dimension_kind"],
            None,
        )?
        .filter(col("fact_family").not_eq(col("dimension_family")))?;
    if rejects_any(invalid).await? {
        return Err(FabricError::PublicationIntegrity(
            "relation violates compiled relation-family rule".into(),
        ));
    }
    Ok(())
}

fn allowed_family_pairs(
    edges: &RecordBatch,
    terms: &RecordBatch,
    predicate: &str,
) -> Result<DataFrame, FabricError> {
    let relation_terms = frame(terms)?
        .filter(col("semantic_type").eq(lit("ontology:relation-kind")))?
        .select(vec![
            cast(col("code_int64"), arrow_schema::DataType::Int32).alias("allowed_kind"),
            col("term_id").alias("allowed_relation_term_id"),
        ])?;
    let family_terms = frame(terms)?
        .filter(col("semantic_type").eq(lit("ontology:entity-family")))?
        .select(vec![
            col("term_id").alias("allowed_family_term_id"),
            cast(col("code_int64"), arrow_schema::DataType::Int16).alias("allowed_family"),
        ])?;
    Ok(frame(edges)?
        .filter(col("predicate_term_id").eq(lit(predicate)))?
        .select(vec![
            col("subject_term_id").alias("relation_term_id"),
            col("object_term_id").alias("family_term_id"),
        ])?
        .join(
            relation_terms,
            JoinType::Inner,
            &["relation_term_id"],
            &["allowed_relation_term_id"],
            None,
        )?
        .join(
            family_terms,
            JoinType::Inner,
            &["family_term_id"],
            &["allowed_family_term_id"],
            None,
        )?
        .select(vec![col("allowed_kind"), col("allowed_family")])?)
}

async fn validate_relation_memberships(
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<(), FabricError> {
    let (Some(relations), Some(entities), Some(edges), Some(terms)) = (
        batches.get(&110),
        batches.get(&100),
        batches.get(&21),
        batches.get(&20),
    ) else {
        return Ok(());
    };
    let relation_rows = frame(relations)?.select(vec![
        col("relation_kind_code").alias("candidate_kind"),
        col("source_id").alias("candidate_source_id"),
        col("target_id").alias("candidate_target_id"),
    ])?;
    let source_entities = frame(entities)?.select(vec![
        col("entity_id").alias("source_entity_id"),
        col("entity_family_code").alias("candidate_source_family"),
    ])?;
    let target_entities = frame(entities)?.select(vec![
        col("entity_id").alias("target_entity_id"),
        col("entity_family_code").alias("candidate_target_family"),
    ])?;
    let candidates = relation_rows
        .join(
            source_entities,
            JoinType::Inner,
            &["candidate_source_id"],
            &["source_entity_id"],
            None,
        )?
        .join(
            target_entities,
            JoinType::Inner,
            &["candidate_target_id"],
            &["target_entity_id"],
            None,
        )?
        .select(vec![
            col("candidate_kind"),
            col("candidate_source_family"),
            col("candidate_target_family"),
        ])?;
    for (candidate_family, predicate) in [
        ("candidate_source_family", "allows_subject_family"),
        ("candidate_target_family", "allows_object_family"),
    ] {
        let allowed = allowed_family_pairs(edges, terms, predicate)?;
        let source = candidates.clone().select(vec![
            col("candidate_kind"),
            col(candidate_family).alias("candidate_family"),
        ])?;
        let invalid = source.join(
            allowed,
            JoinType::LeftAnti,
            &["candidate_kind", "candidate_family"],
            &["allowed_kind", "allowed_family"],
            None,
        )?;
        let invalid = invalid.collect().await?;
        let mut violations = BTreeSet::new();
        for batch in &invalid {
            let kinds = batch
                .column_by_name("candidate_kind")
                .and_then(|column| column.as_any().downcast_ref::<Int32Array>())
                .ok_or_else(|| {
                    FabricError::PublicationIntegrity(
                        "compiled relation membership kind column is not Int32".into(),
                    )
                })?;
            let families = batch
                .column_by_name("candidate_family")
                .and_then(|column| column.as_any().downcast_ref::<Int16Array>())
                .ok_or_else(|| {
                    FabricError::PublicationIntegrity(
                        "compiled relation membership family column is not Int16".into(),
                    )
                })?;
            for row in 0..batch.num_rows() {
                violations.insert((kinds.value(row), families.value(row)));
            }
        }
        if !violations.is_empty() {
            return Err(FabricError::PublicationIntegrity(format!(
                "relation violates compiled {predicate} membership rule: {violations:?}"
            )));
        }
    }
    Ok(())
}

async fn validate_relation_cardinality(
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<(), FabricError> {
    let (Some(relations), Some(kinds)) = (batches.get(&110), batches.get(&14)) else {
        return Ok(());
    };
    let relation_rows = frame(relations)?.select(vec![
        col("relation_kind_code").alias("candidate_kind"),
        col("source_id").alias("candidate_source"),
        col("target_id").alias("candidate_target"),
    ])?;
    for (cardinalities, group_key) in [
        (["many-to-one", "one-to-one"], "candidate_source"),
        (["one-to-many", "one-to-many-ordered"], "candidate_target"),
    ] {
        let constrained = frame(kinds)?
            .filter(
                col("cardinality")
                    .eq(lit(cardinalities[0]))
                    .or(col("cardinality").eq(lit(cardinalities[1]))),
            )?
            .select(vec![col("code").alias("constrained_kind")])?;
        let invalid = relation_rows
            .clone()
            .join(
                constrained,
                JoinType::Inner,
                &["candidate_kind"],
                &["constrained_kind"],
                None,
            )?
            .aggregate(
                vec![col("candidate_kind"), col(group_key)],
                vec![count(lit(1_i64)).alias("relation_count")],
            )?
            .filter(col("relation_count").gt(lit(1_i64)))?;
        if rejects_any(invalid).await? {
            return Err(FabricError::PublicationIntegrity(
                "relation violates compiled cardinality rule".into(),
            ));
        }
    }
    let one_to_one = frame(kinds)?
        .filter(col("cardinality").eq(lit("one-to-one")))?
        .select(vec![col("code").alias("constrained_kind")])?;
    let invalid = relation_rows
        .join(
            one_to_one,
            JoinType::Inner,
            &["candidate_kind"],
            &["constrained_kind"],
            None,
        )?
        .aggregate(
            vec![col("candidate_kind"), col("candidate_target")],
            vec![count(lit(1_i64)).alias("relation_count")],
        )?
        .filter(col("relation_count").gt(lit(1_i64)))?;
    if rejects_any(invalid).await? {
        return Err(FabricError::PublicationIntegrity(
            "relation violates compiled one-to-one target cardinality".into(),
        ));
    }
    Ok(())
}

async fn validate_relation_owners(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    let (Some(relations), Some(entities), Some(kinds)) =
        (batches.get(&110), batches.get(&100), batches.get(&14))
    else {
        return Ok(());
    };
    let relation_rows = frame(relations)?.select(vec![
        col("relation_kind_code").alias("candidate_kind"),
        col("source_id").alias("candidate_source"),
        col("owner_id").alias("candidate_owner"),
    ])?;
    let source_owners = frame(entities)?.select(vec![
        col("entity_id").alias("source_entity_id"),
        col("owner_id").alias("selected_owner"),
    ])?;
    let registered_rules = frame(kinds)?
        .filter(
            [
                "subject-owner",
                "source-file-owner",
                "semantic-owner",
                "source-callable-owner",
                "callable-owner",
                "concurrency-owner",
            ]
            .into_iter()
            .map(|rule| col("owner_selection_rule").eq(lit(rule)))
            .reduce(Expr::or)
            .expect("closed owner rules"),
        )?
        .select(vec![col("code").alias("registered_kind")])?;
    let invalid = relation_rows
        .join(
            registered_rules,
            JoinType::Inner,
            &["candidate_kind"],
            &["registered_kind"],
            None,
        )?
        .join(
            source_owners,
            JoinType::Inner,
            &["candidate_source"],
            &["source_entity_id"],
            None,
        )?
        .filter(col("candidate_owner").not_eq(col("selected_owner")))?;
    if rejects_any(invalid).await? {
        return Err(FabricError::PublicationIntegrity(
            "relation violates compiled owner-selection rule".into(),
        ));
    }
    Ok(())
}

async fn validate_source_spans(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    for (&table_code, batch) in batches {
        if batch.schema().index_of("start_byte").is_err()
            || batch.schema().index_of("end_byte").is_err()
        {
            continue;
        }
        let start = col("start_byte");
        let end = col("end_byte");
        let invalid = frame(batch)?.filter(
            start
                .clone()
                .is_null()
                .not_eq(end.clone().is_null())
                .or(start.clone().lt(lit(0_i64)))
                .or(end.lt(start)),
        )?;
        if rejects_any(invalid).await? {
            let spec = table_spec(table_code).expect("candidate table is generated");
            return Err(FabricError::PublicationIntegrity(format!(
                "{} violates compiled source-span coherence",
                spec.name
            )));
        }
    }
    Ok(())
}

async fn validate_self_edges(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), FabricError> {
    let (Some(relations), Some(kinds)) = (batches.get(&110), batches.get(&14)) else {
        return Ok(());
    };
    let relation = frame(relations)?.filter(col("source_id").eq(col("target_id")))?;
    let forbidden = frame(kinds)?
        .filter(col("self_edge_policy").eq(lit("forbidden")))?
        .select(vec![col("code").alias("forbidden_kind")])?;
    let invalid = relation.join(
        forbidden,
        JoinType::Inner,
        &["relation_kind_code"],
        &["forbidden_kind"],
        None,
    )?;
    if rejects_any(invalid).await? {
        return Err(FabricError::PublicationIntegrity(
            "relation violates compiled self-edge rule".into(),
        ));
    }
    Ok(())
}

/// Execute the closed compiled rule set as DataFusion relational plans.
///
/// # Errors
///
/// Returns an integrity or DataFusion error when any governed key, code, membership,
/// cardinality, owner, property, self-edge, or span rule is violated or cannot execute.
pub async fn validate_compiled_ontology_rules(
    batches: &BTreeMap<i16, RecordBatch>,
) -> Result<(), FabricError> {
    validate_primary_keys(batches).await?;
    validate_governed_codes(batches).await?;
    validate_membership_edges(batches).await?;
    validate_relation_family(batches).await?;
    validate_relation_memberships(batches).await?;
    validate_relation_cardinality(batches).await?;
    validate_relation_owners(batches).await?;
    validate_property_one_of(batches).await?;
    validate_self_edges(batches).await?;
    validate_source_spans(batches).await
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use arrow_array::{Int16Array, Int64Array, RecordBatch, StringArray};
    use arrow_select::concat::concat_batches;

    use super::{
        validate_compiled_ontology_rules, validate_membership_edges, validate_property_one_of,
        validate_source_spans,
    };
    use crate::fact_ingest::{
        EntityRow, FactScope, PropertyFactRow, PropertyValue, encode_entities, encode_properties,
    };
    use crate::ontology_plane::ontology_dimension_batches;
    use crate::schema_registry::{LogicalStructureClass, structure_class};

    fn scope() -> FactScope {
        FactScope {
            workspace_id: [1; 16],
            analysis_context_id: [2; 16],
            source_generation: 1,
            owner_id: [3; 16],
        }
    }

    fn replace_column(
        batch: &RecordBatch,
        name: &str,
        column: Arc<dyn arrow_array::Array>,
    ) -> RecordBatch {
        let mut columns = batch.columns().to_vec();
        columns[batch.schema().index_of(name).expect("generated column")] = column;
        RecordBatch::try_new(batch.schema(), columns).expect("replacement preserves schema")
    }

    #[tokio::test]
    async fn odf_ontology_referential_zero() {
        let batches = ontology_dimension_batches().expect("valid ontology");
        validate_membership_edges(&batches)
            .await
            .expect("generated ontology edge closure");
        for endpoint in ["subject_term_id", "predicate_term_id", "object_term_id"] {
            let mut invalid = batches.clone();
            let edges = invalid.get(&21).expect("ontology edges");
            let unknown = Arc::new(StringArray::from(vec![
                Some("missing-term");
                edges.num_rows()
            ]));
            invalid.insert(21, replace_column(edges, endpoint, unknown));
            assert!(validate_membership_edges(&invalid).await.is_err());
        }
    }

    #[tokio::test]
    async fn odf_ontology_violation_rejection() {
        let mut invalid = ontology_dimension_batches().expect("valid ontology");
        validate_compiled_ontology_rules(&invalid)
            .await
            .expect("generated ontology satisfies compiled rules");
        let dimension = invalid.get(&11).expect("enum domain");
        invalid.insert(
            11,
            concat_batches(&dimension.schema(), [dimension, dimension])
                .expect("duplicate dimension rows"),
        );
        let error = validate_compiled_ontology_rules(&invalid)
            .await
            .expect_err("duplicate primary key must fail");
        assert!(error.to_string().contains("primary-key"));
    }

    #[tokio::test]
    async fn odf_property_value_one_of_gate() {
        let row = PropertyFactRow {
            scope: scope(),
            fact_id: [4; 16],
            subject_entity_id: [5; 16],
            property_kind_code: 10,
            program_point_entity_id: None,
            value: PropertyValue::Integer(42),
            directness_code: 10,
            certainty_code: 10,
            resolution_code: 10,
            producer_code: 10,
            derivation_code: None,
            file_id: None,
            start_byte: None,
            end_byte: None,
            fact_hash64: 7,
        };
        let valid = encode_properties(&[row]).expect("property batch");
        let valid_map = BTreeMap::from([(120, valid.clone())]);
        validate_property_one_of(&valid_map)
            .await
            .expect("matching property tag");
        let invalid = replace_column(
            &valid,
            "value_kind_code",
            Arc::new(Int16Array::from(vec![20_i16])),
        );
        let invalid_map = BTreeMap::from([(120, invalid)]);
        assert!(validate_property_one_of(&invalid_map).await.is_err());
    }

    fn entity_batch() -> RecordBatch {
        encode_entities(&[EntityRow {
            scope: scope(),
            entity_id: [6; 16],
            language: 10,
            entity_family_code: 1,
            entity_kind_code: 10,
            raw_kind_code: None,
            file_id: None,
            start_byte: Some(4),
            end_byte: Some(9),
            name: Some("span-probe".into()),
            qualified_name: None,
            parent_entity_id: None,
            type_id: None,
            flags: 0,
            fact_hash64: 8,
        }])
        .expect("entity batch")
    }

    #[tokio::test]
    async fn odf_span_decision_conformance() {
        assert_eq!(
            structure_class(100, "start_byte"),
            Some(LogicalStructureClass::StructurallyOwnedCohesive)
        );
        let batches = BTreeMap::from([(100, entity_batch())]);
        validate_source_spans(&batches)
            .await
            .expect("coherent flat source span");
    }

    #[tokio::test]
    async fn odf_span_incoherence_rejection() {
        let valid = entity_batch();
        let invalid = replace_column(
            &valid,
            "end_byte",
            Arc::new(Int64Array::from(vec![None::<i64>])),
        );
        let batches = BTreeMap::from([(100, invalid)]);
        assert!(validate_source_spans(&batches).await.is_err());
    }
}
