//! Reproducible Arrow package for the authored ontology program.
//!
//! The package is a generated projection over the schema/phrase authorities. It is
//! publication-neutral: candidate manifests may bind its identities, but Delta versions and
//! activation state never enter the logical or package identity domains.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::Arc;

use arrow_array::{ArrayRef, Int16Array, RecordBatch, StringArray, UInt16Array};
use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use thiserror::Error;

use crate::compiled_ontology::{
    CompiledRuleContract, CompiledRuleOperationKind, compiled_ontology,
};
use crate::model_generated::schema_tables::{SEMANTIC_OPERATION_SPECS, SemanticPredicateOperator};

/// Stable ontology-program package format.
pub const ONTOLOGY_PROGRAM_PACKAGE_VERSION: &str = "ontology-program-package.v1";

/// Packaging choices that may change physical bytes without changing logical program meaning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyPackagingProfile {
    pub profile_id: String,
    pub max_rows_per_batch: usize,
}

impl Default for OntologyPackagingProfile {
    fn default() -> Self {
        Self {
            profile_id: "arrow-ipc-stream.canonical.v1".into(),
            max_rows_per_batch: 1_024,
        }
    }
}

/// One schema-homogeneous Arrow member of the ontology program.
#[derive(Clone, Debug)]
pub struct OntologyProgramMember {
    pub relation_id: String,
    pub schema: SchemaRef,
    pub batches: Vec<RecordBatch>,
    pub ipc_bytes: Vec<u8>,
    pub member_identity: String,
}

/// Acyclic identities and complete member census for one program package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyProgramManifest {
    pub package_version: String,
    pub bootstrap_schema_identity: String,
    pub authored_content_identity: String,
    pub logical_program_identity: String,
    pub packaging_profile_id: String,
    pub member_identities: BTreeMap<String, String>,
    pub package_identity: String,
}

/// Digest-checked package handle admitted by the compiler/session boundary.
#[derive(Clone, Debug)]
pub struct OntologyProgramPackage {
    pub manifest: OntologyProgramManifest,
    pub members: BTreeMap<String, OntologyProgramMember>,
}

/// Publication-neutral seam copied into an external candidate manifest after publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateProgramBinding {
    pub package_identity: String,
    pub logical_program_identity: String,
    pub member_identities: BTreeMap<String, String>,
}

fn manifest_for_members(
    profile_id: &str,
    members: &BTreeMap<String, OntologyProgramMember>,
) -> Result<OntologyProgramManifest, OntologyProgramError> {
    let bootstrap_schema_identity = framed(
        members
            .values()
            .map(|member| format!("{}:{:?}", member.relation_id, member.schema))
            .map(String::into_bytes),
    );
    let ontology = compiled_ontology();
    let authored_content_identity = framed([
        ontology.phrase_authority.canonical_digest.as_bytes(),
        ontology.query_form_authority.canonical_digest.as_bytes(),
        crate::schema_registry::schema_contract_digest().as_bytes(),
    ]);
    let logical_program_identity = framed(
        std::iter::once(bootstrap_schema_identity.as_bytes().to_vec())
            .chain(std::iter::once(
                authored_content_identity.as_bytes().to_vec(),
            ))
            .chain(
                members
                    .values()
                    .map(logical_rows)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
    );
    let member_identities = members
        .iter()
        .map(|(name, member)| (name.clone(), member.member_identity.clone()))
        .collect::<BTreeMap<_, _>>();
    let package_identity = framed(
        [
            ONTOLOGY_PROGRAM_PACKAGE_VERSION.as_bytes().to_vec(),
            logical_program_identity.as_bytes().to_vec(),
            profile_id.as_bytes().to_vec(),
        ]
        .into_iter()
        .chain(
            member_identities
                .iter()
                .map(|(name, digest)| format!("{name}:{digest}").into_bytes()),
        ),
    );
    Ok(OntologyProgramManifest {
        package_version: ONTOLOGY_PROGRAM_PACKAGE_VERSION.into(),
        bootstrap_schema_identity,
        authored_content_identity,
        logical_program_identity,
        packaging_profile_id: profile_id.to_owned(),
        member_identities,
        package_identity,
    })
}

impl From<&OntologyProgramPackage> for CandidateProgramBinding {
    fn from(package: &OntologyProgramPackage) -> Self {
        Self {
            package_identity: package.manifest.package_identity.clone(),
            logical_program_identity: package.manifest.logical_program_identity.clone(),
            member_identities: package.manifest.member_identities.clone(),
        }
    }
}

/// Typed package build/admission failures.
#[derive(Debug, Error)]
pub enum OntologyProgramError {
    #[error(transparent)]
    Arrow(#[from] ArrowError),
    #[error("ONTOLOGY_PROGRAM_CONTRACT_INVALID:{0}")]
    Contract(String),
    #[error("ONTOLOGY_PROGRAM_DIGEST_MISMATCH:{0}")]
    Digest(String),
    #[error("ONTOLOGY_PROGRAM_RESOURCE_LIMIT:{0}")]
    Resource(String),
}

fn framed(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut bytes = Vec::new();
    for part in parts {
        let part = part.as_ref();
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    crate::integrity::framed_digest(&bytes)
}

fn operation_name(operation: CompiledRuleOperationKind) -> &'static str {
    match operation {
        CompiledRuleOperationKind::ForeignKeyAntiJoin => "foreign_key_anti_join",
        CompiledRuleOperationKind::GovernedCodeAntiJoin => "governed_code_anti_join",
        CompiledRuleOperationKind::PrimaryKeyUniquenessAggregate => {
            "primary_key_uniqueness_aggregate"
        }
        CompiledRuleOperationKind::IdDomainConformance => "id_domain_conformance",
        CompiledRuleOperationKind::OntologyMembershipAntiJoin => "ontology_membership_anti_join",
        CompiledRuleOperationKind::RelationFamilyConformanceJoin => {
            "relation_family_conformance_join"
        }
        CompiledRuleOperationKind::RelationCardinalityAggregate => "relation_cardinality_aggregate",
        CompiledRuleOperationKind::RelationOwnerConformanceJoin => {
            "relation_owner_conformance_join"
        }
        CompiledRuleOperationKind::RelationSelfEdgeJoin => "relation_self_edge_join",
        CompiledRuleOperationKind::PropertyValueOneOf => "property_value_one_of",
        CompiledRuleOperationKind::SourceSpanAllOrNone => "source_span_all_or_none",
    }
}

fn phrase_calculation(operator: SemanticPredicateOperator) -> &'static str {
    match operator {
        SemanticPredicateOperator::Equals => "calculation.phrase-equals.v1",
        SemanticPredicateOperator::InSet => "calculation.phrase-in-set.v1",
    }
}

fn rule_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("operation_id", DataType::Utf8, false),
        Field::new("operation_kind", DataType::Utf8, false),
        Field::new("operand_ordinal", DataType::UInt16, false),
        Field::new("relation_ref", DataType::Utf8, false),
        Field::new("column_ref", DataType::Utf8, false),
        Field::new("logical_type", DataType::Utf8, false),
        Field::new("calculation_id", DataType::Utf8, false),
        Field::new("policy_id", DataType::Utf8, false),
        Field::new("expected_result_contract", DataType::Utf8, false),
        Field::new("diagnostic_code", DataType::Utf8, false),
    ]))
}

fn rule_batch(rules: &[CompiledRuleContract]) -> Result<RecordBatch, OntologyProgramError> {
    let mut operation_ids = Vec::new();
    let mut operation_kinds = Vec::new();
    let mut ordinals = Vec::new();
    let mut relations = Vec::new();
    let mut columns = Vec::new();
    let mut logical_types = Vec::new();
    let mut calculations = Vec::new();
    let mut policies = Vec::new();
    let mut outputs = Vec::new();
    let mut diagnostics = Vec::new();
    for rule in rules {
        if rule.ordered_operands.is_empty()
            || rule.calculation_id.is_empty()
            || rule.policy_id.is_empty()
        {
            return Err(OntologyProgramError::Contract(format!(
                "{} is a bare executable record",
                rule.rule_id
            )));
        }
        for (index, operand) in rule.ordered_operands.iter().enumerate() {
            if usize::from(operand.ordinal) != index
                || operand.relation_ref.is_empty()
                || operand.column_ref.is_empty()
                || operand.logical_type.is_empty()
            {
                return Err(OntologyProgramError::Contract(format!(
                    "{} has an invalid ordered operand",
                    rule.rule_id
                )));
            }
            operation_ids.push(rule.rule_id);
            operation_kinds.push(operation_name(rule.operation_kind));
            ordinals.push(operand.ordinal);
            relations.push(operand.relation_ref);
            columns.push(operand.column_ref);
            logical_types.push(operand.logical_type);
            calculations.push(rule.calculation_id);
            policies.push(rule.policy_id);
            outputs.push(rule.output_contract);
            diagnostics.push(rule.diagnostic_code);
        }
    }
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(operation_ids)),
        Arc::new(StringArray::from(operation_kinds)),
        Arc::new(UInt16Array::from(ordinals)),
        Arc::new(StringArray::from(relations)),
        Arc::new(StringArray::from(columns)),
        Arc::new(StringArray::from(logical_types)),
        Arc::new(StringArray::from(calculations)),
        Arc::new(StringArray::from(policies)),
        Arc::new(StringArray::from(outputs)),
        Arc::new(StringArray::from(diagnostics)),
    ];
    Ok(RecordBatch::try_new(rule_schema(), columns)?)
}

fn phrase_batch() -> Result<RecordBatch, OntologyProgramError> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("phrase_id", DataType::Utf8, false),
        Field::new("canonical_text", DataType::Utf8, false),
        Field::new("column_ref", DataType::Utf8, false),
        Field::new("operation_kind", DataType::Utf8, false),
        Field::new("operand_domain", DataType::Utf8, false),
        Field::new("operand_code", DataType::Int16, false),
        Field::new("calculation_id", DataType::Utf8, false),
        Field::new("expected_result_contract", DataType::Utf8, false),
        Field::new("diagnostic_code", DataType::Utf8, false),
    ]));
    let mut phrases = Vec::new();
    let mut texts = Vec::new();
    let mut columns = Vec::new();
    let mut operations = Vec::new();
    let mut domains = Vec::new();
    let mut codes = Vec::new();
    let mut calculations = Vec::new();
    let mut outputs = Vec::new();
    let mut diagnostics = Vec::new();
    for operation in SEMANTIC_OPERATION_SPECS {
        if operation.operand_codes.is_empty() {
            return Err(OntologyProgramError::Contract(format!(
                "{} has no governed operand",
                operation.phrase_id
            )));
        }
        for code in operation.operand_codes {
            phrases.push(operation.phrase_id);
            texts.push(operation.canonical_text);
            columns.push(operation.column_role);
            operations.push(match operation.operator {
                SemanticPredicateOperator::Equals => "equals",
                SemanticPredicateOperator::InSet => "in_set",
            });
            domains.push(operation.operand_domain);
            codes.push(*code);
            calculations.push(phrase_calculation(operation.operator));
            outputs.push(operation.output_role);
            diagnostics.push(operation.diagnostic_code);
        }
    }
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(phrases)),
            Arc::new(StringArray::from(texts)),
            Arc::new(StringArray::from(columns)),
            Arc::new(StringArray::from(operations)),
            Arc::new(StringArray::from(domains)),
            Arc::new(Int16Array::from(codes)),
            Arc::new(StringArray::from(calculations)),
            Arc::new(StringArray::from(outputs)),
            Arc::new(StringArray::from(diagnostics)),
        ],
    )?)
}

fn calculation_batch(rules: &[CompiledRuleContract]) -> Result<RecordBatch, OntologyProgramError> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("calculation_id", DataType::Utf8, false),
        Field::new("engine", DataType::Utf8, false),
        Field::new("native_operation", DataType::Utf8, false),
        Field::new("return_contract", DataType::Utf8, false),
    ]));
    let mut catalog = BTreeMap::new();
    for rule in rules {
        if catalog
            .insert(
                rule.calculation_id,
                (operation_name(rule.operation_kind), rule.output_contract),
            )
            .is_some()
        {
            return Err(OntologyProgramError::Contract(format!(
                "duplicate calculation identity {}",
                rule.calculation_id
            )));
        }
    }
    for operation in SEMANTIC_OPERATION_SPECS {
        catalog
            .entry(phrase_calculation(operation.operator))
            .or_insert((
                match operation.operator {
                    SemanticPredicateOperator::Equals => "eq",
                    SemanticPredicateOperator::InSet => "in_list",
                },
                "predicate",
            ));
    }
    let ids = catalog.keys().copied().collect::<Vec<_>>();
    let operations = catalog.values().map(|value| value.0).collect::<Vec<_>>();
    let returns = catalog.values().map(|value| value.1).collect::<Vec<_>>();
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(ids)),
            Arc::new(StringArray::from(vec!["datafusion-native"; catalog.len()])),
            Arc::new(StringArray::from(operations)),
            Arc::new(StringArray::from(returns)),
        ],
    )?)
}

fn bootstrap_batch() -> Result<RecordBatch, OntologyProgramError> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("ordinal", DataType::UInt16, false),
        Field::new("family_id", DataType::Utf8, false),
        Field::new("binding_kind", DataType::Utf8, false),
        Field::new("relation_id", DataType::Utf8, false),
        Field::new("depends_on", DataType::Utf8, true),
    ]));
    let rows = [
        ("codes", "authored", "authority.codes", None),
        (
            "edges_memberships",
            "authored",
            "authority.edges_memberships",
            Some("codes"),
        ),
        (
            "semantic_types",
            "authored",
            "authority.semantic_types",
            Some("codes"),
        ),
        (
            "table_contracts",
            "authored",
            "authority.table_contracts",
            Some("semantic_types"),
        ),
        (
            "column_contracts",
            "authored",
            "authority.column_contracts",
            Some("table_contracts"),
        ),
        (
            "result_contracts",
            "authored",
            "authority.result_contracts",
            Some("column_contracts"),
        ),
        (
            "identity_recipes",
            "authored",
            "authority.identity_recipes",
            Some("semantic_types"),
        ),
        (
            "phrase_bindings",
            "program_member",
            "program.phrase_operation",
            Some("codes"),
        ),
        (
            "calculation_bindings",
            "program_member",
            "program.calculation_catalog",
            Some("phrase_bindings"),
        ),
        (
            "rule_bindings",
            "program_member",
            "program.rule_operation",
            Some("calculation_bindings"),
        ),
        (
            "snapshot_identity",
            "candidate",
            "candidate.snapshot",
            Some("table_contracts"),
        ),
        (
            "publication_identity",
            "candidate",
            "candidate.publication",
            Some("snapshot_identity"),
        ),
        (
            "plan_identity",
            "candidate",
            "candidate.plan",
            Some("rule_bindings"),
        ),
        (
            "package_identity",
            "candidate",
            "candidate.package",
            Some("rule_bindings"),
        ),
        (
            "policy_identity",
            "candidate",
            "candidate.policy",
            Some("package_identity"),
        ),
        (
            "exact_table_identities",
            "candidate",
            "candidate.exact_tables",
            Some("publication_identity"),
        ),
    ];
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt16Array::from_iter_values((0..rows.len()).map(
                |ordinal| u16::try_from(ordinal).expect("bootstrap ordinal"),
            ))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|row| row.0))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|row| row.1))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|row| row.2))),
            Arc::new(StringArray::from(
                rows.iter().map(|row| row.3).collect::<Vec<_>>(),
            )),
        ],
    )?)
}

fn encode_member(
    relation_id: &str,
    batches: Vec<RecordBatch>,
) -> Result<OntologyProgramMember, OntologyProgramError> {
    let schema = batches
        .first()
        .ok_or_else(|| OntologyProgramError::Contract(format!("{relation_id} is empty")))?
        .schema();
    if batches.iter().any(|batch| batch.schema() != schema) {
        return Err(OntologyProgramError::Contract(format!(
            "{relation_id} contains heterogeneous schemas"
        )));
    }
    let mut ipc_bytes = Vec::new();
    {
        let mut writer = StreamWriter::try_new(&mut ipc_bytes, schema.as_ref())?;
        for batch in &batches {
            writer.write(batch)?;
        }
        writer.finish()?;
    }
    let member_identity = framed([relation_id.as_bytes(), ipc_bytes.as_slice()]);
    Ok(OntologyProgramMember {
        relation_id: relation_id.into(),
        schema,
        batches,
        ipc_bytes,
        member_identity,
    })
}

fn logical_rows(member: &OntologyProgramMember) -> Result<Vec<u8>, OntologyProgramError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(member.relation_id.as_bytes());
    for batch in &member.batches {
        bytes.extend_from_slice(format!("{batch:?}").as_bytes());
    }
    Ok(bytes)
}

/// Build one deterministic, publication-neutral ontology-program package.
///
/// # Errors
///
/// Rejects malformed generated contracts, empty/heterogeneous members, Arrow encoding failure,
/// or a packaging profile that cannot preserve at least one complete canonical batch.
pub fn build_ontology_program_package(
    profile: &OntologyPackagingProfile,
) -> Result<OntologyProgramPackage, OntologyProgramError> {
    if profile.profile_id.trim().is_empty() || profile.max_rows_per_batch == 0 {
        return Err(OntologyProgramError::Resource(
            "packaging profile is empty or admits zero rows".into(),
        ));
    }
    let rules = compiled_ontology().rules;
    let mut members: BTreeMap<String, OntologyProgramMember> = BTreeMap::new();
    for (relation_id, batch) in [
        ("program.bootstrap", bootstrap_batch()?),
        ("program.rule_operation", rule_batch(rules)?),
        ("program.phrase_operation", phrase_batch()?),
        ("program.calculation_catalog", calculation_batch(rules)?),
    ] {
        if batch.num_rows() > profile.max_rows_per_batch {
            return Err(OntologyProgramError::Resource(format!(
                "{relation_id} has {} rows beyond profile limit {}",
                batch.num_rows(),
                profile.max_rows_per_batch
            )));
        }
        members.insert(relation_id.into(), encode_member(relation_id, vec![batch])?);
    }

    let manifest = manifest_for_members(&profile.profile_id, &members)?;
    let package = OntologyProgramPackage { manifest, members };
    validate_ontology_program_package(&package)?;
    Ok(package)
}

/// Verify all member bytes, schemas, batches, and manifest identities before use.
///
/// # Errors
///
/// Returns a digest or contract error when any package byte or census was altered.
pub fn validate_ontology_program_package(
    package: &OntologyProgramPackage,
) -> Result<(), OntologyProgramError> {
    if package.members.keys().collect::<BTreeSet<_>>()
        != package.manifest.member_identities.keys().collect()
    {
        return Err(OntologyProgramError::Contract(
            "member census differs from manifest".into(),
        ));
    }
    for (name, member) in &package.members {
        let expected = framed([name.as_bytes(), member.ipc_bytes.as_slice()]);
        if member.relation_id != *name
            || member.member_identity != expected
            || package.manifest.member_identities.get(name) != Some(&expected)
        {
            return Err(OntologyProgramError::Digest(name.clone()));
        }
        let decoded = StreamReader::try_new(Cursor::new(&member.ipc_bytes), None)?
            .collect::<Result<Vec<_>, _>>()?;
        if decoded != member.batches {
            return Err(OntologyProgramError::Contract(format!(
                "{name} IPC round-trip changed rows"
            )));
        }
    }
    let expected = manifest_for_members(&package.manifest.packaging_profile_id, &package.members)?;
    if package.manifest != expected {
        return Err(OntologyProgramError::Digest(
            "package manifest identity closure".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn reseal_ontology_program_package(
    package: &mut OntologyProgramPackage,
) -> Result<(), OntologyProgramError> {
    package.manifest =
        manifest_for_members(&package.manifest.packaging_profile_id, &package.members)?;
    validate_ontology_program_package(package)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        CandidateProgramBinding, OntologyPackagingProfile, build_ontology_program_package,
        validate_ontology_program_package,
    };

    #[test]
    fn ontology_program_bundle_semantic_parity() {
        let package = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("compiled package");
        assert_eq!(package.members.len(), 4);
        let rules = &package.members["program.rule_operation"];
        assert_eq!(
            rules.batches[0].num_rows(),
            crate::compiled_ontology::compiled_ontology()
                .rules
                .iter()
                .map(|rule| rule.ordered_operands.len())
                .sum::<usize>()
        );
        assert!(
            crate::compiled_ontology::compiled_ontology()
                .rules
                .iter()
                .all(|rule| !rule.calculation_id.is_empty()
                    && !rule.policy_id.is_empty()
                    && !rule.ordered_operands.is_empty())
        );
    }

    #[test]
    fn ontology_program_bundle_digest_acyclicity() {
        let first = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("first package");
        let alternate = build_ontology_program_package(&OntologyPackagingProfile {
            profile_id: "arrow-ipc-stream.canonical.v2".into(),
            ..OntologyPackagingProfile::default()
        })
        .expect("alternate physical profile");
        assert_eq!(
            first.manifest.logical_program_identity,
            alternate.manifest.logical_program_identity
        );
        assert_ne!(
            first.manifest.package_identity,
            alternate.manifest.package_identity
        );
        let binding = CandidateProgramBinding::from(&first);
        assert_eq!(binding.package_identity, first.manifest.package_identity);
    }

    #[test]
    fn ontology_program_bundle_ipc_reproducibility() {
        let first = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("first package");
        let second = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("second package");
        assert_eq!(first.manifest, second.manifest);
        assert!(
            first
                .members
                .iter()
                .all(|(name, member)| { member.ipc_bytes == second.members[name].ipc_bytes })
        );
        let mut corrupted = first.clone();
        corrupted
            .members
            .get_mut("program.bootstrap")
            .expect("bootstrap")
            .ipc_bytes
            .push(0);
        assert!(validate_ontology_program_package(&corrupted).is_err());
    }

    #[test]
    fn ontology_program_bundle_model_rebuild() {
        let package = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("compiled package");
        let calculation_rows = package.members["program.calculation_catalog"].batches[0].num_rows();
        let calculation_ids = crate::compiled_ontology::compiled_ontology()
            .rules
            .iter()
            .map(|rule| rule.calculation_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(calculation_rows, calculation_ids.len() + 2);
        assert!(
            package
                .manifest
                .authored_content_identity
                .starts_with("b3:")
        );
    }
}
