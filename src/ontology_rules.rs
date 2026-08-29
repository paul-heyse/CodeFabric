//! Execution boundary for normalized ontology programs.

use std::collections::BTreeMap;

use arrow_array::RecordBatch;

use crate::fabric::FabricError;
use crate::governed_session::GovernedSession;
use crate::ontology_executor::OntologyProgramCompiler;

/// Compile and execute every package-selected validation program exactly once.
///
/// Every row-level rule is represented by typed plan/expression relations in the package. This
/// boundary contains no rule-specific dispatch or fallback validator.
///
/// # Errors
///
/// Returns an integrity error when the package graph, provider binding, governed analysis,
/// execution, resource envelope, or empty-violation contract fails.
pub async fn execute_ontology_program(
    batches: &BTreeMap<i16, RecordBatch>,
    package: &crate::ontology_program::OntologyProgramPackage,
    session: &GovernedSession,
) -> Result<(), FabricError> {
    let compiler = OntologyProgramCompiler::decode(package)
        .map_err(|error| FabricError::PublicationIntegrity(error.to_string()))?;
    let providers = crate::ontology_relational_program::candidate_batch_providers(batches)
        .map_err(|error| FabricError::PublicationIntegrity(error.to_string()))?;
    let programs = compiler
        .relational_program
        .programs()
        .values()
        .filter(|program| program.execution_phase == "candidate_validation")
        .collect::<Vec<_>>();
    if programs.is_empty() {
        return Err(FabricError::PublicationIntegrity(
            "compiled ontology package has no validation programs".into(),
        ));
    }
    for program in programs {
        let plan = compiler
            .relational_program
            .compile(&program.program_id, &providers)
            .map_err(|error| FabricError::PublicationIntegrity(error.to_string()))?;
        let governed = session
            .seal_plan(plan)
            .map_err(|error| FabricError::PublicationIntegrity(error.to_string()))?;
        let outcome = session
            .execute_gate(
                &governed,
                &format!("ontology-program-validation:{}", program.program_id),
                "candidate:publication",
                &program.program_id,
                &crate::ontology_gate::GateResourceEnvelope::default(),
            )
            .await
            .map_err(|error| FabricError::PublicationIntegrity(error.to_string()))?;
        if outcome.receipt.gate_checksum.row_count != 0 {
            return Err(FabricError::PublicationIntegrity(format!(
                "{}:{}:compiled ontology program rejected {} violation rows",
                program.diagnostic_code,
                program.program_id,
                outcome.receipt.gate_checksum.row_count
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use arrow_array::{RecordBatch, StringArray};
    use arrow_select::concat::concat_batches;
    use datafusion::prelude::SessionConfig;

    use super::execute_ontology_program;
    use crate::governed_session::GovernedSession;
    use crate::ontology_plane::ontology_dimension_batches;
    use crate::ontology_program::{OntologyPackagingProfile, build_ontology_program_package};
    use crate::schema_registry::table_specs;

    fn complete_candidate() -> BTreeMap<i16, RecordBatch> {
        let mut batches = table_specs()
            .iter()
            .map(|spec| {
                (
                    spec.table_code,
                    RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)),
                )
            })
            .collect::<BTreeMap<_, _>>();
        batches.extend(ontology_dimension_batches().expect("ontology dimensions"));
        batches
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

    async fn execute(batches: &BTreeMap<i16, RecordBatch>) -> Result<(), String> {
        let package = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("program package");
        let session = GovernedSession::new(SessionConfig::new(), "policy:test:ontology-program")
            .expect("governed session");
        execute_ontology_program(batches, &package, &session)
            .await
            .map_err(|error| error.to_string())
    }

    #[tokio::test]
    async fn odf_ontology_referential_zero() {
        let valid = complete_candidate();
        execute(&valid).await.expect("valid ontology program");
        for endpoint in ["subject_term_id", "predicate_term_id", "object_term_id"] {
            let mut invalid = valid.clone();
            let edges = invalid.get(&21).expect("ontology edges");
            let unknown = Arc::new(StringArray::from(vec![
                Some("missing-term");
                edges.num_rows()
            ]));
            invalid.insert(21, replace_column(edges, endpoint, unknown));
            assert!(execute(&invalid).await.is_err(), "{endpoint}");
        }
    }

    #[tokio::test]
    async fn odf_ontology_violation_rejection() {
        let mut invalid = complete_candidate();
        let dimension = invalid.get(&11).expect("enum domain");
        invalid.insert(
            11,
            concat_batches(&dimension.schema(), [dimension, dimension])
                .expect("duplicate dimension rows"),
        );
        assert!(execute(&invalid).await.is_err());
    }
}
