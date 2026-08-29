//! Generic DataFusion decoder and lowerer for a digest-checked ontology program package.

use std::collections::{BTreeMap, BTreeSet};

use arrow_array::{Array as _, Int16Array, RecordBatch, StringArray};
use datafusion::logical_expr::{Expr, col, lit};
use datafusion::scalar::ScalarValue;
use thiserror::Error;

use crate::ontology_program::{
    OntologyProgramError, OntologyProgramPackage, validate_ontology_program_package,
};

/// One authored rule bound to its normalized relational program and calculation contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedRuleBinding {
    pub rule_id: String,
    pub calculation_id: String,
    pub policy_id: String,
    pub input_contract: String,
    pub expected_result_contract: String,
    pub determinism_class: String,
    pub diagnostic_code: String,
}

/// One fail-closed semantic phrase binding compiled through the same calculation catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPhraseBinding {
    pub phrase_id: String,
    pub canonical_text: String,
    pub column_ref: String,
    pub operation_kind: String,
    pub operand_domain: String,
    pub operand_logical_type: String,
    pub operand_codes: Vec<i16>,
    pub null_policy: String,
    pub calculation_id: String,
    pub expected_result_contract: String,
    pub diagnostic_code: String,
}

/// One governed query phrase decoded from Arrow, including all accepted labels and modifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedQueryPhrase {
    pub phrase_id: String,
    pub canonical_text: String,
    pub accepted_aliases: Vec<String>,
    pub plan_node_kind: String,
    pub output_role: String,
    pub contract_family: String,
    pub contract_code: String,
    pub required_modifiers: Vec<String>,
}

/// One native DataFusion calculation; `engine` must remain `datafusion-native`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCalculation {
    pub calculation_id: String,
    pub engine: String,
    pub native_operation: String,
    pub return_contract: String,
}

/// Typed decoder/lowering failures.
#[derive(Debug, Error)]
pub enum OntologyProgramCompileError {
    #[error(transparent)]
    Package(#[from] OntologyProgramError),
    #[error("ONTOLOGY_PROGRAM_DECODE_INVALID:{0}")]
    Decode(String),
    #[error("ONTOLOGY_PROGRAM_UNSUPPORTED:{0}")]
    Unsupported(String),
    #[error("SEMANTIC_PHRASE_UNSUPPORTED:{0}")]
    Phrase(String),
}

fn utf8<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a StringArray, OntologyProgramCompileError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| OntologyProgramCompileError::Decode(format!("{name} is not Utf8")))
}

fn uint16<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a arrow_array::UInt16Array, OntologyProgramCompileError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<arrow_array::UInt16Array>())
        .ok_or_else(|| OntologyProgramCompileError::Decode(format!("{name} is not UInt16")))
}

fn int16<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a Int16Array, OntologyProgramCompileError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<Int16Array>())
        .ok_or_else(|| OntologyProgramCompileError::Decode(format!("{name} is not Int16")))
}

fn one_batch<'a>(
    package: &'a OntologyProgramPackage,
    relation: &str,
) -> Result<&'a RecordBatch, OntologyProgramCompileError> {
    let member = package
        .members
        .get(relation)
        .ok_or_else(|| OntologyProgramCompileError::Decode(format!("missing {relation}")))?;
    if member.batches.len() != 1 {
        return Err(OntologyProgramCompileError::Decode(format!(
            "{relation} must have one canonical batch"
        )));
    }
    Ok(&member.batches[0])
}

fn decode_rules(
    package: &OntologyProgramPackage,
) -> Result<BTreeMap<String, DecodedRuleBinding>, OntologyProgramCompileError> {
    let batch = one_batch(package, "program.rule_binding")?;
    let rule_ids = utf8(batch, "rule_id")?;
    let calculations = utf8(batch, "calculation_id")?;
    let policies = utf8(batch, "policy_id")?;
    let inputs = utf8(batch, "input_contract")?;
    let outputs = utf8(batch, "expected_result_contract")?;
    let determinism = utf8(batch, "determinism_class")?;
    let diagnostics = utf8(batch, "diagnostic_code")?;
    let mut rules = BTreeMap::new();
    for row in 0..batch.num_rows() {
        let rule = DecodedRuleBinding {
            rule_id: rule_ids.value(row).into(),
            calculation_id: calculations.value(row).into(),
            policy_id: policies.value(row).into(),
            input_contract: inputs.value(row).into(),
            expected_result_contract: outputs.value(row).into(),
            determinism_class: determinism.value(row).into(),
            diagnostic_code: diagnostics.value(row).into(),
        };
        if rule.rule_id.is_empty()
            || rule.calculation_id.is_empty()
            || rule.policy_id.is_empty()
            || rule.input_contract.is_empty()
            || rule.expected_result_contract.is_empty()
            || rule.determinism_class.is_empty()
            || rule.diagnostic_code.is_empty()
            || rules.insert(rule.rule_id.clone(), rule).is_some()
        {
            return Err(OntologyProgramCompileError::Decode(format!(
                "rule binding row {row} is empty or duplicated"
            )));
        }
    }
    Ok(rules)
}

fn decode_phrases(
    package: &OntologyProgramPackage,
) -> Result<BTreeMap<String, DecodedPhraseBinding>, OntologyProgramCompileError> {
    let batch = one_batch(package, "program.phrase_binding")?;
    let ids = utf8(batch, "phrase_id")?;
    let texts = utf8(batch, "canonical_text")?;
    let columns = utf8(batch, "column_ref")?;
    let operations = utf8(batch, "operation_kind")?;
    let domains = utf8(batch, "operand_domain")?;
    let logical_types = utf8(batch, "operand_logical_type")?;
    let codes = int16(batch, "operand_code")?;
    let null_policies = utf8(batch, "null_policy")?;
    let calculations = utf8(batch, "calculation_id")?;
    let outputs = utf8(batch, "expected_result_contract")?;
    let diagnostics = utf8(batch, "diagnostic_code")?;
    let mut phrases: BTreeMap<String, DecodedPhraseBinding> = BTreeMap::new();
    for row in 0..batch.num_rows() {
        let phrase_id = ids.value(row);
        let phrase = phrases
            .entry(phrase_id.into())
            .or_insert_with(|| DecodedPhraseBinding {
                phrase_id: phrase_id.into(),
                canonical_text: texts.value(row).into(),
                column_ref: columns.value(row).into(),
                operation_kind: operations.value(row).into(),
                operand_domain: domains.value(row).into(),
                operand_logical_type: logical_types.value(row).into(),
                operand_codes: Vec::new(),
                null_policy: null_policies.value(row).into(),
                calculation_id: calculations.value(row).into(),
                expected_result_contract: outputs.value(row).into(),
                diagnostic_code: diagnostics.value(row).into(),
            });
        if phrase.canonical_text != texts.value(row)
            || phrase.column_ref != columns.value(row)
            || phrase.operation_kind != operations.value(row)
            || phrase.operand_logical_type != logical_types.value(row)
            || phrase.calculation_id != calculations.value(row)
        {
            return Err(OntologyProgramCompileError::Decode(format!(
                "{phrase_id} has inconsistent rows"
            )));
        }
        phrase.operand_codes.push(codes.value(row));
    }
    for phrase in phrases.values_mut() {
        phrase.operand_codes.sort_unstable();
        if phrase.operand_codes.is_empty()
            || phrase
                .operand_codes
                .windows(2)
                .any(|pair| pair[0] == pair[1])
        {
            return Err(OntologyProgramCompileError::Decode(format!(
                "{} has empty or duplicate operands",
                phrase.phrase_id
            )));
        }
    }
    Ok(phrases)
}

fn decode_query_phrases(
    package: &OntologyProgramPackage,
) -> Result<BTreeMap<String, DecodedQueryPhrase>, OntologyProgramCompileError> {
    let batch = one_batch(package, "program.query_phrase")?;
    let ids = utf8(batch, "phrase_id")?;
    let texts = utf8(batch, "canonical_text")?;
    let node_kinds = utf8(batch, "plan_node_kind")?;
    let output_roles = utf8(batch, "output_role")?;
    let contract_families = utf8(batch, "contract_family")?;
    let contract_codes = utf8(batch, "contract_code")?;
    let mut phrases = BTreeMap::new();
    for row in 0..batch.num_rows() {
        let phrase = DecodedQueryPhrase {
            phrase_id: ids.value(row).into(),
            canonical_text: texts.value(row).into(),
            accepted_aliases: Vec::new(),
            plan_node_kind: node_kinds.value(row).into(),
            output_role: output_roles.value(row).into(),
            contract_family: contract_families.value(row).into(),
            contract_code: contract_codes.value(row).into(),
            required_modifiers: Vec::new(),
        };
        if phrase.phrase_id.is_empty()
            || phrase.canonical_text.is_empty()
            || phrases.insert(phrase.phrase_id.clone(), phrase).is_some()
        {
            return Err(OntologyProgramCompileError::Decode(
                "query phrase identity is empty or duplicated".into(),
            ));
        }
    }

    let aliases = one_batch(package, "program.query_phrase_alias")?;
    let alias_ids = utf8(aliases, "phrase_id")?;
    let alias_ordinals = uint16(aliases, "alias_ordinal")?;
    let alias_texts = utf8(aliases, "alias_text")?;
    for row in 0..aliases.num_rows() {
        let phrase = phrases.get_mut(alias_ids.value(row)).ok_or_else(|| {
            OntologyProgramCompileError::Decode(format!(
                "query phrase alias references {}",
                alias_ids.value(row)
            ))
        })?;
        if usize::from(alias_ordinals.value(row)) != phrase.accepted_aliases.len()
            || alias_texts.value(row).is_empty()
        {
            return Err(OntologyProgramCompileError::Decode(format!(
                "{} has unordered or empty aliases",
                phrase.phrase_id
            )));
        }
        phrase.accepted_aliases.push(alias_texts.value(row).into());
    }

    let modifiers = one_batch(package, "program.query_phrase_modifier")?;
    let modifier_ids = utf8(modifiers, "phrase_id")?;
    let modifier_ordinals = uint16(modifiers, "modifier_ordinal")?;
    let modifier_values = utf8(modifiers, "modifier")?;
    for row in 0..modifiers.num_rows() {
        let phrase = phrases.get_mut(modifier_ids.value(row)).ok_or_else(|| {
            OntologyProgramCompileError::Decode(format!(
                "query phrase modifier references {}",
                modifier_ids.value(row)
            ))
        })?;
        if usize::from(modifier_ordinals.value(row)) != phrase.required_modifiers.len()
            || modifier_values.value(row).is_empty()
        {
            return Err(OntologyProgramCompileError::Decode(format!(
                "{} has unordered or empty modifiers",
                phrase.phrase_id
            )));
        }
        phrase
            .required_modifiers
            .push(modifier_values.value(row).into());
    }
    Ok(phrases)
}

fn decode_query_projections(
    package: &OntologyProgramPackage,
) -> Result<BTreeMap<(String, String), Vec<i32>>, OntologyProgramCompileError> {
    let batch = one_batch(package, "program.query_projection")?;
    let phrase_ids = utf8(batch, "phrase_id")?;
    let target_kinds = utf8(batch, "target_kind")?;
    let ordinals = uint16(batch, "operand_ordinal")?;
    let codes = batch
        .column_by_name("operand_code")
        .and_then(|column| column.as_any().downcast_ref::<arrow_array::Int32Array>())
        .ok_or_else(|| OntologyProgramCompileError::Decode("operand_code is not Int32".into()))?;
    let mut projections: BTreeMap<(String, String), Vec<i32>> = BTreeMap::new();
    for row in 0..batch.num_rows() {
        let key = (
            phrase_ids.value(row).to_owned(),
            target_kinds.value(row).to_owned(),
        );
        let operands = projections.entry(key.clone()).or_default();
        if usize::from(ordinals.value(row)) != operands.len() || codes.is_null(row) {
            return Err(OntologyProgramCompileError::Decode(format!(
                "query projection {}:{} has unordered or null operands",
                key.0, key.1
            )));
        }
        operands.push(codes.value(row));
    }
    Ok(projections)
}

fn decode_calculations(
    package: &OntologyProgramPackage,
) -> Result<BTreeMap<String, DecodedCalculation>, OntologyProgramCompileError> {
    let batch = one_batch(package, "program.calculation_contract")?;
    let ids = utf8(batch, "calculation_id")?;
    let engines = utf8(batch, "engine")?;
    let operations = utf8(batch, "native_operation")?;
    let returns = utf8(batch, "return_contract")?;
    let mut calculations = BTreeMap::new();
    for row in 0..batch.num_rows() {
        let calculation = DecodedCalculation {
            calculation_id: ids.value(row).into(),
            engine: engines.value(row).into(),
            native_operation: operations.value(row).into(),
            return_contract: returns.value(row).into(),
        };
        if calculation.engine != "datafusion-native"
            || calculations
                .insert(calculation.calculation_id.clone(), calculation)
                .is_some()
        {
            return Err(OntologyProgramCompileError::Decode(
                "calculation catalog is not a native bijection".into(),
            ));
        }
    }
    Ok(calculations)
}

/// Digest-checked generic compiler for the current native DataFusion profile.
#[derive(Clone, Debug)]
pub struct OntologyProgramCompiler {
    pub package_identity: String,
    pub rules: BTreeMap<String, DecodedRuleBinding>,
    pub relational_program: crate::ontology_relational_program::OntologyRelationalProgram,
    pub phrases: BTreeMap<String, DecodedPhraseBinding>,
    pub query_phrases: BTreeMap<String, DecodedQueryPhrase>,
    pub query_projection_codes: BTreeMap<(String, String), Vec<i32>>,
    pub calculations: BTreeMap<String, DecodedCalculation>,
}

impl OntologyProgramCompiler {
    fn lower_decoded_phrase(
        &self,
        phrase: &DecodedPhraseBinding,
        qualifier: Option<&str>,
    ) -> Result<Expr, OntologyProgramCompileError> {
        if phrase.expected_result_contract != "predicate" {
            return Err(OntologyProgramCompileError::Unsupported(format!(
                "{}:{}",
                phrase.diagnostic_code, phrase.expected_result_contract
            )));
        }
        let calculation = self
            .calculations
            .get(&phrase.calculation_id)
            .ok_or_else(|| {
                OntologyProgramCompileError::Unsupported(phrase.calculation_id.clone())
            })?;
        let column_ref = qualifier.map_or_else(
            || phrase.column_ref.clone(),
            |qualifier| format!("{qualifier}.{}", phrase.column_ref),
        );
        let column = col(column_ref);
        let values = phrase
            .operand_codes
            .iter()
            .map(|code| match phrase.operand_logical_type.as_str() {
                "int16" => Ok(lit(ScalarValue::Int16(Some(*code)))),
                "int32" => Ok(lit(ScalarValue::Int32(Some(i32::from(*code))))),
                logical_type => Err(OntologyProgramCompileError::Unsupported(format!(
                    "{}:{logical_type}",
                    phrase.diagnostic_code
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let predicate = match calculation.native_operation.as_str() {
            "eq" if values.len() == 1 => column.clone().eq(values[0].clone()),
            "in_list" if !values.is_empty() => column.clone().in_list(values, false),
            operation => return Err(OntologyProgramCompileError::Unsupported(operation.into())),
        };
        match phrase.null_policy.as_str() {
            "unknown_is_false" => Ok(predicate.is_true()),
            "reject_unknown" => Ok(column.is_not_null().and(predicate)),
            policy => Err(OntologyProgramCompileError::Unsupported(format!(
                "{}:{policy}",
                phrase.diagnostic_code
            ))),
        }
    }

    /// Decode and cross-link one package before any expression is lowered.
    ///
    /// # Errors
    ///
    /// Rejects digest drift, missing/duplicate relations, dangling calculation identities,
    /// non-native engines, malformed operands, or unsupported current-profile calculations.
    pub fn decode(package: &OntologyProgramPackage) -> Result<Self, OntologyProgramCompileError> {
        validate_ontology_program_package(package)?;
        let rules = decode_rules(package)?;
        let relational_program =
            crate::ontology_relational_program::OntologyRelationalProgram::decode(package)?;
        let phrases = decode_phrases(package)?;
        let query_phrases = decode_query_phrases(package)?;
        let query_projection_codes = decode_query_projections(package)?;
        let calculations = decode_calculations(package)?;
        let referenced = rules
            .values()
            .map(|rule| rule.calculation_id.as_str())
            .chain(
                phrases
                    .values()
                    .map(|phrase| phrase.calculation_id.as_str()),
            )
            .chain(
                relational_program
                    .programs()
                    .values()
                    .map(|program| program.calculation_id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let available = calculations
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if !referenced.is_subset(&available) {
            return Err(OntologyProgramCompileError::Decode(format!(
                "dangling calculations: {:?}",
                referenced.difference(&available).collect::<Vec<_>>()
            )));
        }
        let supported = BTreeSet::from(["eq", "in_list", "relational_program"]);
        if let Some(calculation) = calculations
            .values()
            .find(|calculation| !supported.contains(calculation.native_operation.as_str()))
        {
            return Err(OntologyProgramCompileError::Unsupported(
                calculation.native_operation.clone(),
            ));
        }
        for program in relational_program.programs().values().filter(|program| {
            matches!(
                program.execution_phase.as_str(),
                "candidate_validation" | "semantic_analysis"
            )
        }) {
            let rule = rules.get(&program.rule_id).ok_or_else(|| {
                OntologyProgramCompileError::Decode(format!(
                    "program {} has no authored rule binding {}",
                    program.program_id, program.rule_id
                ))
            })?;
            let calculation = calculations.get(&program.calculation_id).ok_or_else(|| {
                OntologyProgramCompileError::Decode(format!(
                    "program {} has no calculation {}",
                    program.program_id, program.calculation_id
                ))
            })?;
            if rule.calculation_id != program.calculation_id
                || rule.policy_id != program.policy_id
                || rule.expected_result_contract != program.expected_result_contract
                || rule.diagnostic_code != program.diagnostic_code
                || calculation.native_operation != "relational_program"
            {
                return Err(OntologyProgramCompileError::Decode(format!(
                    "program {} drifts from its rule/calculation authority: calculation={:?}/{:?}, policy={:?}/{:?}, result={:?}/{:?}, diagnostic={:?}/{:?}, native={:?}",
                    program.program_id,
                    program.calculation_id,
                    rule.calculation_id,
                    program.policy_id,
                    rule.policy_id,
                    program.expected_result_contract,
                    rule.expected_result_contract,
                    program.diagnostic_code,
                    rule.diagnostic_code,
                    calculation.native_operation,
                )));
            }
        }
        Ok(Self {
            package_identity: package.manifest.package_identity.clone(),
            rules,
            relational_program,
            phrases,
            query_phrases,
            query_projection_codes,
            calculations,
        })
    }

    /// Lower one governed phrase to ordinary typed DataFusion expressions.
    ///
    /// # Errors
    ///
    /// Unknown phrases and any calculation outside the current native profile fail closed.
    pub fn lower_phrase(&self, phrase_id: &str) -> Result<Expr, OntologyProgramCompileError> {
        let phrase = self
            .phrases
            .get(phrase_id)
            .ok_or_else(|| OntologyProgramCompileError::Phrase(phrase_id.into()))?;
        self.lower_decoded_phrase(phrase, None)
    }

    /// Resolve canonical phrase text through the package; no literal fallback is allowed.
    ///
    /// # Errors
    ///
    /// Returns an error when the phrase is absent or cannot be lowered by the catalog.
    pub fn lower_phrase_text(&self, text: &str) -> Result<Expr, OntologyProgramCompileError> {
        let phrase = self
            .phrases
            .values()
            .find(|phrase| phrase.canonical_text == text)
            .ok_or_else(|| OntologyProgramCompileError::Phrase(text.into()))?;
        self.lower_phrase(&phrase.phrase_id)
    }

    /// Resolve and lower canonical phrase text against a qualified relational input.
    ///
    /// # Errors
    ///
    /// Returns an error when the phrase is absent or cannot be lowered by the catalog.
    pub fn lower_phrase_text_for_qualifier(
        &self,
        text: &str,
        qualifier: &str,
    ) -> Result<Expr, OntologyProgramCompileError> {
        let phrase = self
            .phrases
            .values()
            .find(|phrase| phrase.canonical_text == text)
            .ok_or_else(|| OntologyProgramCompileError::Phrase(text.into()))?;
        self.lower_decoded_phrase(phrase, Some(qualifier))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{
        Array as _, ArrayRef, Int16Array, Int32Array, RecordBatch, StringArray, UInt64Array,
    };
    use arrow_schema::{DataType, Field, Schema};
    use datafusion::datasource::{MemTable, provider_as_source};
    use datafusion::logical_expr::LogicalPlanBuilder;
    use datafusion::prelude::SessionConfig;

    use super::{OntologyProgramCompileError, OntologyProgramCompiler};
    use crate::governed_session::GovernedSession;
    use crate::ontology_gate::{GateResourceEnvelope, OntologyGateOutcome};
    use crate::ontology_program::{
        OntologyPackagingProfile, OntologyProgramPackage, build_ontology_program_package,
        replace_program_utf8_cell,
    };

    fn compiler() -> OntologyProgramCompiler {
        let package = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("program package");
        OntologyProgramCompiler::decode(&package).expect("program compiler")
    }

    fn empty_candidate_providers()
    -> std::collections::BTreeMap<String, Arc<dyn datafusion::catalog::TableProvider>> {
        let batches = crate::schema_registry::table_specs()
            .iter()
            .map(|spec| {
                (
                    spec.table_code,
                    RecordBatch::new_empty(Arc::clone(&spec.arrow_schema)),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        crate::ontology_relational_program::candidate_batch_providers(&batches)
            .expect("schema-faithful providers")
    }

    fn compiled_candidate_plans(
        compiler: &OntologyProgramCompiler,
    ) -> Result<std::collections::BTreeMap<String, String>, OntologyProgramCompileError> {
        let providers = empty_candidate_providers();
        compiler
            .relational_program
            .programs()
            .values()
            .filter(|program| program.execution_phase == "candidate_validation")
            .map(|program| {
                Ok((
                    program.program_id.clone(),
                    compiler
                        .relational_program
                        .compile(&program.program_id, &providers)?
                        .display_indent()
                        .to_string(),
                ))
            })
            .collect()
    }

    fn utf8_row(
        package: &OntologyProgramPackage,
        relation: &str,
        column: &str,
        predicate: impl Fn(&str) -> bool,
    ) -> usize {
        let batch = &package.members[relation].batches[0];
        let values = batch
            .column_by_name(column)
            .and_then(|value| value.as_any().downcast_ref::<StringArray>())
            .expect("Utf8 program column");
        (0..values.len())
            .find(|&row| !values.is_null(row) && predicate(values.value(row)))
            .unwrap_or_else(|| panic!("no matching {relation}.{column} row"))
    }

    fn assert_resealed_mutation_causal(
        base: &OntologyProgramPackage,
        base_plans: &std::collections::BTreeMap<String, String>,
        relation: &str,
        column: &str,
        row: usize,
        replacement: &str,
    ) {
        let mut mutated = base.clone();
        replace_program_utf8_cell(&mut mutated, relation, column, row, replacement)
            .expect("resealed causal mutant");
        assert_ne!(
            mutated.manifest.package_identity, base.manifest.package_identity,
            "{relation}.{column} did not affect package authority"
        );
        if let Ok(compiler) = OntologyProgramCompiler::decode(&mutated)
            && let Ok(mutated_plans) = compiled_candidate_plans(&compiler)
        {
            assert_ne!(
                &mutated_plans, base_plans,
                "{relation}.{column} mutation did not change or reject planning"
            );
        }
    }

    async fn execute_phrase_once(
        compiler: &OntologyProgramCompiler,
        phrase_id: &str,
        codes: Vec<Option<i16>>,
    ) -> OntologyGateOutcome {
        let phrase = compiler.phrases.get(phrase_id).expect("compiled phrase");
        let mut metadata = std::collections::HashMap::new();
        metadata.insert(
            "com.codefabric.cpg.semantic_type".into(),
            format!("enum:{}", phrase.operand_domain),
        );
        let row_count = codes.len();
        let code_values: ArrayRef = match phrase.operand_logical_type.as_str() {
            "int16" => Arc::new(Int16Array::from(codes)),
            "int32" => Arc::new(Int32Array::from(
                codes
                    .into_iter()
                    .map(|value| value.map(i32::from))
                    .collect::<Vec<_>>(),
            )),
            logical_type => panic!("unsupported phrase fixture type {logical_type}"),
        };
        let schema = Arc::new(Schema::new(vec![
            Field::new("row_id", DataType::UInt64, false),
            Field::new(
                &phrase.column_ref,
                match phrase.operand_logical_type.as_str() {
                    "int16" => DataType::Int16,
                    "int32" => DataType::Int32,
                    logical_type => panic!("unsupported phrase fixture type {logical_type}"),
                },
                true,
            )
            .with_metadata(metadata),
        ]));
        let row_ids = (0..row_count)
            .map(|value| u64::try_from(value).expect("fixture row index"))
            .collect::<Vec<_>>();
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(UInt64Array::from(row_ids)), code_values],
        )
        .expect("phrase fixture batch");
        let provider = Arc::new(MemTable::try_new(schema, vec![vec![batch]]).expect("fixture"));
        let plan = LogicalPlanBuilder::scan("phrase_fixture", provider_as_source(provider), None)
            .expect("fixture scan")
            .filter(compiler.lower_phrase(phrase_id).expect("lower phrase"))
            .expect("phrase filter")
            .build()
            .expect("phrase plan");
        let session = GovernedSession::new(SessionConfig::new(), "policy.ontology.test.v1")
            .expect("governed phrase session");
        let sealed = session.seal_plan(plan).expect("seal phrase plan");
        session
            .execute_gate(
                &sealed,
                &format!("execution:{phrase_id}"),
                "candidate:ontology-program",
                phrase_id,
                &GateResourceEnvelope::default(),
            )
            .await
            .expect("execute phrase once")
    }

    #[tokio::test]
    async fn ontology_compiled_program_native_profile() {
        let compiler = compiler();
        assert_eq!(compiler.rules.len(), 11);
        assert!(compiler.phrases.len() >= 3);
        assert!(compiler.phrases.values().all(|phrase| {
            !phrase.column_ref.is_empty()
                && !phrase.operand_domain.is_empty()
                && !phrase.operand_codes.is_empty()
                && phrase.expected_result_contract == "predicate"
        }));
        assert!(compiler.calculations.values().all(|calculation| {
            calculation.engine == "datafusion-native" && !calculation.native_operation.is_empty()
        }));
        let execution = execute_phrase_once(
            &compiler,
            "CONDITION_CERTAINTY_EXACT",
            vec![Some(10), Some(40), Some(50), None],
        )
        .await;
        assert_eq!(execution.batches[0].num_rows(), 2);
        assert!(execution.receipt.gate_checksum.checksum.starts_with("b3:"));
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)] // One causality matrix mutates every normalized program operand family.
    async fn ontology_compiled_program_causality_matrix() {
        let compiler = compiler();
        for phrase in compiler.phrases.values() {
            let accepted = phrase.operand_codes[0];
            let rejected = (i16::MIN..=i16::MAX)
                .find(|candidate| !phrase.operand_codes.contains(candidate))
                .expect("unregistered operand");
            let selected = execute_phrase_once(
                &compiler,
                &phrase.phrase_id,
                vec![Some(accepted), Some(rejected)],
            )
            .await;
            assert_eq!(selected.batches[0].num_rows(), 1);
        }
        assert!(compiler.rules.values().all(|rule| {
            !rule.rule_id.is_empty()
                && !rule.policy_id.is_empty()
                && !rule.expected_result_contract.is_empty()
                && !rule.input_contract.is_empty()
        }));

        let base = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("causal package");
        let base_compiler = OntologyProgramCompiler::decode(&base).expect("base compiler");
        let base_plans = compiled_candidate_plans(&base_compiler).expect("base plans");

        let binary_row = utf8_row(&base, "program.binary_expr", "operator", |value| {
            value == "eq"
        });
        let mut changed = base.clone();
        replace_program_utf8_cell(
            &mut changed,
            "program.binary_expr",
            "operator",
            binary_row,
            "neq",
        )
        .expect("resealed operator mutation");
        let changed_compiler = OntologyProgramCompiler::decode(&changed).expect("changed compiler");
        assert_ne!(
            base_plans,
            compiled_candidate_plans(&changed_compiler).expect("changed plans"),
            "binary operator mutation did not change any governed plan"
        );

        let program_row = utf8_row(
            &base,
            "program.program_contract",
            "execution_phase",
            |value| value == "candidate_validation",
        );
        for (column, replacement) in [
            ("rule_id", "ontology.unknown.v1"),
            ("root_node_id", "plan.unknown"),
            ("calculation_id", "calculation.unknown.v1"),
            ("policy_id", "policy.unknown.v1"),
            ("expected_result_contract", "unknown-result.v1"),
            ("diagnostic_code", "UNKNOWN_DIAGNOSTIC"),
        ] {
            assert_resealed_mutation_causal(
                &base,
                &base_plans,
                "program.program_contract",
                column,
                program_row,
                replacement,
            );
        }

        for (relation, column, replacement) in [
            ("program.scan_node", "relation_ref", "table:32767"),
            ("program.scan_node", "relation_alias", "unknown_scan_alias"),
            ("program.filter_node", "predicate_expr_id", "expr.unknown"),
            ("program.join_node", "join_type", "unsupported_join"),
            ("program.column_expr", "column_name", "unknown_column"),
            (
                "program.column_expr",
                "relation_alias",
                "unknown_column_alias",
            ),
            ("program.binary_expr", "operator", "unsupported_binary"),
            ("program.call_expr", "function_name", "unsupported_call"),
            ("program.cast_expr", "target_type", "unsupported_type"),
            ("program.plan_edge", "child_node_id", "plan.unknown"),
            ("program.expression_edge", "child_expr_id", "expr.unknown"),
            ("program.expression_edge", "role", "unsupported_role"),
        ] {
            assert_resealed_mutation_causal(&base, &base_plans, relation, column, 0, replacement);
        }

        let literal_row = utf8_row(&base, "program.literal_expr", "value", |_| true);
        let mut literal_changed = base.clone();
        replace_program_utf8_cell(
            &mut literal_changed,
            "program.literal_expr",
            "value",
            literal_row,
            "causal-mutant",
        )
        .expect("resealed literal mutation");
        let literal_result = OntologyProgramCompiler::decode(&literal_changed)
            .and_then(|compiler| compiled_candidate_plans(&compiler));
        assert!(
            literal_result.is_err() || literal_result.expect("literal result") != base_plans,
            "literal mutation did not affect planning"
        );
    }

    #[test]
    fn ontology_phrase_binding_fail_closed() {
        let compiler = compiler();
        assert!(compiler.lower_phrase("UNKNOWN_PHRASE").is_err());
        assert!(compiler.lower_phrase_text("certainty is guessed").is_err());
        assert!(compiler.lower_phrase_text("certainty is exact").is_ok());
    }

    #[test]
    fn ontology_calculation_catalog_bijection() {
        let compiler = compiler();
        let referenced = compiler
            .rules
            .values()
            .map(|rule| rule.calculation_id.as_str())
            .chain(
                compiler
                    .phrases
                    .values()
                    .map(|phrase| phrase.calculation_id.as_str()),
            )
            .chain(
                compiler
                    .relational_program
                    .programs()
                    .values()
                    .map(|program| program.calculation_id.as_str()),
            )
            .collect::<std::collections::BTreeSet<_>>();
        let available = compiler
            .calculations
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(referenced, available);
    }
}
