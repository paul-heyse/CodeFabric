//! Deterministic composition and frozen projections for lane-owned semantic fragments.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use super::repository_model::read_stable;

pub const FRAGMENT_PATHS: [&str; 3] = [
    "contracts/semantic-fragments/shared.json",
    "contracts/semantic-fragments/python.json",
    "contracts/semantic-fragments/rust.json",
];
pub const JSON_PROJECTION_PATH: &str =
    "contracts/generated/model/schema/semantic-lane-fragments.json";
pub const RUST_PROJECTION_PATH: &str = "src/generated/model_semantic_lane_fragments.rs";
const MAX_FRAGMENT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticLane {
    Shared,
    Python,
    Rust,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FragmentModel {
    owned_table_codes: Vec<i16>,
    owned_observation_schema_ids: Vec<String>,
    table_additions: Vec<Value>,
    table_scope_additions: Vec<Value>,
    semantic_type_binding_additions: Vec<Value>,
    provider_observation_schema_additions: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EnumValueReference {
    domain: String,
    codes: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FragmentRegistry {
    property_codes: Vec<u64>,
    enum_values: Vec<EnumValueReference>,
    property_record_additions: Vec<Value>,
    enum_domain_additions: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IngestFragment {
    pub contract_id: String,
    pub observation_schema_ids: Vec<String>,
    pub output_table_codes: Vec<i16>,
    pub required_fields: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFragment {
    pub contract_id: String,
    pub context_kinds: Vec<String>,
    pub discovery_port: String,
    pub partition_keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InvalidationFragment {
    pub contract_id: String,
    pub trigger_kinds: Vec<String>,
    pub invalidated_table_codes: Vec<i16>,
    pub scope: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticFragmentDocument {
    artifact_id: String,
    artifact_kind: String,
    version: String,
    compatible_suite_major: u64,
    status: String,
    canonical_digest: String,
    fragment_id: String,
    lane: SemanticLane,
    owner_packet: String,
    model: FragmentModel,
    registry: FragmentRegistry,
    ingest: Vec<IngestFragment>,
    contexts: Vec<ContextFragment>,
    invalidations: Vec<InvalidationFragment>,
}

#[derive(Clone, Debug)]
pub struct SemanticFragmentSet {
    documents: Vec<SemanticFragmentDocument>,
    source_digests: BTreeMap<String, String>,
}

impl SemanticFragmentSet {
    pub fn load(repository_root: &Path) -> Result<Self, SemanticFragmentError> {
        let mut documents = Vec::new();
        let mut source_digests = BTreeMap::new();
        for (expected_lane, path) in [
            (SemanticLane::Shared, FRAGMENT_PATHS[0]),
            (SemanticLane::Python, FRAGMENT_PATHS[1]),
            (SemanticLane::Rust, FRAGMENT_PATHS[2]),
        ] {
            let bytes = read_stable(&repository_root.join(path), MAX_FRAGMENT_BYTES)?;
            let document: SemanticFragmentDocument = serde_json::from_slice(&bytes)?;
            if document.lane != expected_lane {
                return Err(SemanticFragmentError::Invalid(format!(
                    "{path} declares the wrong lane"
                )));
            }
            let digest = detached_digest(&document)?;
            if document.canonical_digest != digest {
                return Err(SemanticFragmentError::Invalid(format!(
                    "{path} canonical digest is stale: declared {}, computed {digest}",
                    document.canonical_digest
                )));
            }
            source_digests.insert(path.to_owned(), digest);
            documents.push(document);
        }
        let set = Self {
            documents,
            source_digests,
        };
        set.validate()?;
        Ok(set)
    }

    fn validate(&self) -> Result<(), SemanticFragmentError> {
        let mut artifact_ids = BTreeSet::new();
        let mut fragment_ids = BTreeSet::new();
        let mut owned_tables = BTreeSet::new();
        let mut owned_observations = BTreeSet::new();
        let mut contract_ids = BTreeSet::new();
        for document in &self.documents {
            if document.artifact_kind != "semantic-lane-fragment"
                || document.version != "1.0"
                || document.compatible_suite_major != 1
                || document.status != "accepted"
                || !document.owner_packet.starts_with("WP")
                || !artifact_ids.insert(document.artifact_id.as_str())
                || !fragment_ids.insert(document.fragment_id.as_str())
            {
                return Err(SemanticFragmentError::Invalid(format!(
                    "invalid fragment header {}",
                    document.fragment_id
                )));
            }
            for code in &document.model.owned_table_codes {
                if !owned_tables.insert(*code) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "table {code} has multiple lane owners"
                    )));
                }
            }
            for addition in &document.model.table_additions {
                let code = scalar_i16(addition, "table_code")?;
                if !document.model.owned_table_codes.contains(&code) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} adds table {code} without owning it",
                        document.fragment_id
                    )));
                }
            }
            for addition in &document.model.table_scope_additions {
                let code = scalar_i16(addition, "table_code")?;
                if !document.model.owned_table_codes.contains(&code) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} adds scope for table {code} without owning it",
                        document.fragment_id
                    )));
                }
            }
            for schema_id in &document.model.owned_observation_schema_ids {
                if !owned_observations.insert(schema_id.as_str()) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "observation schema {schema_id} has multiple lane owners"
                    )));
                }
            }
            for addition in &document.model.provider_observation_schema_additions {
                let schema_id = scalar_str(addition, "schema_id")?;
                if !document
                    .model
                    .owned_observation_schema_ids
                    .iter()
                    .any(|owned| owned == schema_id)
                {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} adds observation schema {schema_id} without owning it",
                        document.fragment_id
                    )));
                }
            }
            for addition in &document.registry.property_record_additions {
                let code = scalar_u64(addition, "property_code")?;
                if !document.registry.property_codes.contains(&code) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} adds property {code} without owning it",
                        document.fragment_id
                    )));
                }
            }
            for addition in &document.registry.enum_domain_additions {
                let domain = scalar_str(addition, "domain")?;
                let Some(reference) = document
                    .registry
                    .enum_values
                    .iter()
                    .find(|reference| reference.domain == domain)
                else {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} adds enum domain {domain} without owning values in it",
                        document.fragment_id
                    )));
                };
                for value in array(addition, "values")? {
                    let code = scalar_u64(value, "code")?;
                    if !reference.codes.contains(&code) {
                        return Err(SemanticFragmentError::Invalid(format!(
                            "{} adds {domain} code {code} without owning it",
                            document.fragment_id
                        )));
                    }
                }
            }
            for contract_id in document
                .ingest
                .iter()
                .map(|item| item.contract_id.as_str())
                .chain(
                    document
                        .contexts
                        .iter()
                        .map(|item| item.contract_id.as_str()),
                )
                .chain(
                    document
                        .invalidations
                        .iter()
                        .map(|item| item.contract_id.as_str()),
                )
            {
                if contract_id.is_empty() || !contract_ids.insert(contract_id) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "duplicate or empty semantic contract {contract_id}"
                    )));
                }
            }
            if document.ingest.is_empty()
                || document.contexts.is_empty()
                || document.invalidations.is_empty()
                || document.ingest.iter().any(|item| {
                    item.observation_schema_ids.is_empty()
                        || item.output_table_codes.is_empty()
                        || item.required_fields.is_empty()
                })
                || document.contexts.iter().any(|item| {
                    item.context_kinds.is_empty()
                        || item.discovery_port
                            != "crate::analysis_context::AnalysisContextDiscoveryPort"
                        || item.partition_keys
                            != ["workspace_id", "analysis_context_id", "source_generation"]
                })
                || document.invalidations.iter().any(|item| {
                    item.trigger_kinds.is_empty()
                        || item.invalidated_table_codes.is_empty()
                        || item.scope.is_empty()
                })
            {
                return Err(SemanticFragmentError::Invalid(format!(
                    "{} has an incomplete runtime contract",
                    document.fragment_id
                )));
            }
        }
        Ok(())
    }

    pub fn compose_schema(&self, schema: &mut Value) -> Result<(), SemanticFragmentError> {
        for document in &self.documents {
            merge_records(
                array_mut(schema, "tables")?,
                &document.model.table_additions,
                "table_code",
            )?;
            merge_records(
                array_mut(schema, "table_scopes")?,
                &document.model.table_scope_additions,
                "table_code",
            )?;
            merge_records(
                array_mut(schema, "semantic_type_bindings")?,
                &document.model.semantic_type_binding_additions,
                "semantic_type",
            )?;
            merge_records(
                array_mut(schema, "provider_observation_schemas")?,
                &document.model.provider_observation_schema_additions,
                "schema_id",
            )?;
        }
        sort_records(array_mut(schema, "tables")?, "table_code")?;
        sort_records(array_mut(schema, "table_scopes")?, "table_code")?;
        sort_records(
            array_mut(schema, "semantic_type_bindings")?,
            "semantic_type",
        )?;
        sort_records(
            array_mut(schema, "provider_observation_schemas")?,
            "observation_family_code",
        )?;
        self.validate_schema_ownership(schema)
    }

    /// Compose lane-owned property and enum additions into effective registry projections.
    pub fn compose_registries(
        &self,
        property_registry: &mut Value,
        enum_registry: &mut Value,
    ) -> Result<(), SemanticFragmentError> {
        for document in &self.documents {
            merge_records(
                array_mut(property_registry, "records")?,
                &document.registry.property_record_additions,
                "property_code",
            )?;
            for addition in &document.registry.enum_domain_additions {
                merge_enum_domain(array_mut(enum_registry, "records")?, addition)?;
            }
        }
        sort_records(array_mut(property_registry, "records")?, "property_code")?;
        sort_records(array_mut(enum_registry, "records")?, "domain")?;
        self.validate_registry_values(property_registry, enum_registry)
    }

    fn validate_schema_ownership(&self, schema: &Value) -> Result<(), SemanticFragmentError> {
        let tables = keyed_records(array(schema, "tables")?, "table_code")?;
        let observations =
            keyed_records(array(schema, "provider_observation_schemas")?, "schema_id")?;
        for document in &self.documents {
            for code in &document.model.owned_table_codes {
                if !tables.contains_key(&RecordKey::Signed(i64::from(*code))) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} owns absent table {code}",
                        document.fragment_id
                    )));
                }
            }
            for schema_id in &document.model.owned_observation_schema_ids {
                if !observations.contains_key(&RecordKey::Text(schema_id.clone())) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} owns absent observation schema {schema_id}",
                        document.fragment_id
                    )));
                }
            }
            for ingest in &document.ingest {
                for table_code in &ingest.output_table_codes {
                    if !tables.contains_key(&RecordKey::Signed(i64::from(*table_code))) {
                        return Err(SemanticFragmentError::Invalid(format!(
                            "{} targets absent output table {table_code}",
                            ingest.contract_id
                        )));
                    }
                }
                let available_fields = if document.lane == SemanticLane::Shared {
                    ingest
                        .output_table_codes
                        .iter()
                        .filter_map(|code| {
                            tables.get(&RecordKey::Signed(i64::from(*code))).copied()
                        })
                        .map(record_field_names)
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flatten()
                        .collect::<BTreeSet<_>>()
                } else {
                    ingest
                        .observation_schema_ids
                        .iter()
                        .filter_map(|schema_id| {
                            observations
                                .get(&RecordKey::Text(schema_id.clone()))
                                .copied()
                        })
                        .map(record_field_names)
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flatten()
                        .collect::<BTreeSet<_>>()
                };
                if let Some(field) = ingest
                    .required_fields
                    .iter()
                    .find(|field| !available_fields.contains(field.as_str()))
                {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} requires absent field {field}",
                        ingest.contract_id
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn validate_registry_values(
        &self,
        property_registry: &Value,
        enum_registry: &Value,
    ) -> Result<(), SemanticFragmentError> {
        let property_codes = array(property_registry, "records")?
            .iter()
            .filter_map(|record| record.get("property_code").and_then(Value::as_u64))
            .collect::<BTreeSet<_>>();
        let enum_values = array(enum_registry, "records")?
            .iter()
            .filter_map(|record| {
                Some((
                    record.get("domain")?.as_str()?.to_owned(),
                    record
                        .get("values")?
                        .as_array()?
                        .iter()
                        .filter_map(|value| value.get("code").and_then(Value::as_u64))
                        .collect::<BTreeSet<_>>(),
                ))
            })
            .collect::<BTreeMap<_, _>>();
        for document in &self.documents {
            for code in &document.registry.property_codes {
                if !property_codes.contains(code) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} references absent property code {code}",
                        document.fragment_id
                    )));
                }
            }
            for reference in &document.registry.enum_values {
                let Some(codes) = enum_values.get(&reference.domain) else {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} references absent enum domain {}",
                        document.fragment_id, reference.domain
                    )));
                };
                if let Some(code) = reference.codes.iter().find(|code| !codes.contains(code)) {
                    return Err(SemanticFragmentError::Invalid(format!(
                        "{} references absent {} code {code}",
                        document.fragment_id, reference.domain
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn render_json(&self) -> Result<Vec<u8>, SemanticFragmentError> {
        let value = json!({
            "artifact_id": "codefabric.generated.semantic-lane-fragments",
            "artifact_kind": "generated-projection",
            "version": "1.0",
            "compatible_suite_major": 1,
            "status": "draft",
            "source_digests": self.source_digests,
            "fragments": self.documents,
        });
        let mut bytes = serde_json::to_vec_pretty(&value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn composed_source_digest(
        &self,
        base_source_digest: &str,
    ) -> Result<String, SemanticFragmentError> {
        let canonical = codefabric::contracts::jcs::canonicalize_value(&json!({
            "base_source_digest": base_source_digest,
            "fragment_source_digests": self.source_digests,
        }))?;
        Ok(codefabric::integrity::framed_digest(&canonical))
    }

    pub fn render_rust(&self) -> String {
        let mut output = String::from(
            "// @generated from lane-owned semantic fragments; do not edit.\n\n\
             #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
             pub enum SemanticLane { Shared, Python, Rust }\n\
             #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
             pub struct SemanticIngestContract { pub contract_id: &'static str, pub lane: SemanticLane, pub observation_schema_ids: &'static [&'static str], pub output_table_codes: &'static [i16], pub required_fields: &'static [&'static str] }\n\
             #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
             pub struct SemanticContextContract { pub contract_id: &'static str, pub lane: SemanticLane, pub context_kinds: &'static [&'static str], pub discovery_port: &'static str, pub partition_keys: &'static [&'static str] }\n\
             #[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
             pub struct SemanticInvalidationContract { pub contract_id: &'static str, pub lane: SemanticLane, pub trigger_kinds: &'static [&'static str], pub invalidated_table_codes: &'static [i16], pub scope: &'static str }\n\n",
        );
        render_ingest(&mut output, &self.documents);
        render_contexts(&mut output, &self.documents);
        render_invalidations(&mut output, &self.documents);
        output
    }
}

fn render_ingest(output: &mut String, documents: &[SemanticFragmentDocument]) {
    output.push_str("pub const SEMANTIC_INGEST_CONTRACTS: &[SemanticIngestContract] = &[\n");
    for document in documents {
        for item in &document.ingest {
            writeln!(output, "    SemanticIngestContract {{ contract_id: {:?}, lane: SemanticLane::{:?}, observation_schema_ids: &{:?}, output_table_codes: &{:?}, required_fields: &{:?} }},", item.contract_id, document.lane, item.observation_schema_ids, item.output_table_codes, item.required_fields).unwrap();
        }
    }
    output.push_str("];\n\n");
}

fn render_contexts(output: &mut String, documents: &[SemanticFragmentDocument]) {
    output.push_str("pub const SEMANTIC_CONTEXT_CONTRACTS: &[SemanticContextContract] = &[\n");
    for document in documents {
        for item in &document.contexts {
            writeln!(output, "    SemanticContextContract {{ contract_id: {:?}, lane: SemanticLane::{:?}, context_kinds: &{:?}, discovery_port: {:?}, partition_keys: &{:?} }},", item.contract_id, document.lane, item.context_kinds, item.discovery_port, item.partition_keys).unwrap();
        }
    }
    output.push_str("];\n\n");
}

fn render_invalidations(output: &mut String, documents: &[SemanticFragmentDocument]) {
    output.push_str(
        "pub const SEMANTIC_INVALIDATION_CONTRACTS: &[SemanticInvalidationContract] = &[\n",
    );
    for document in documents {
        for item in &document.invalidations {
            writeln!(output, "    SemanticInvalidationContract {{ contract_id: {:?}, lane: SemanticLane::{:?}, trigger_kinds: &{:?}, invalidated_table_codes: &{:?}, scope: {:?} }},", item.contract_id, document.lane, item.trigger_kinds, item.invalidated_table_codes, item.scope).unwrap();
        }
    }
    output.push_str("];\n");
}

fn detached_digest(document: &SemanticFragmentDocument) -> Result<String, SemanticFragmentError> {
    let mut value = serde_json::to_value(document)?;
    value
        .as_object_mut()
        .ok_or_else(|| SemanticFragmentError::Invalid("fragment root is not an object".into()))?
        .remove("canonical_digest");
    let canonical = codefabric::contracts::jcs::canonicalize_value(&value)?;
    Ok(codefabric::integrity::framed_digest(&canonical))
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, SemanticFragmentError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| SemanticFragmentError::Invalid(format!("schema {field} is not an array")))
}

fn array_mut<'a>(
    value: &'a mut Value,
    field: &str,
) -> Result<&'a mut Vec<Value>, SemanticFragmentError> {
    value
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| SemanticFragmentError::Invalid(format!("schema {field} is not an array")))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RecordKey {
    Signed(i64),
    Unsigned(u64),
    Text(String),
}

fn record_key(record: &Value, key: &str) -> Result<RecordKey, SemanticFragmentError> {
    let value = record
        .get(key)
        .ok_or_else(|| SemanticFragmentError::Invalid(format!("record has no {key}")))?;
    if let Some(value) = value.as_i64() {
        return Ok(RecordKey::Signed(value));
    }
    if let Some(value) = value.as_u64() {
        return Ok(RecordKey::Unsigned(value));
    }
    value.as_str().map_or_else(
        || {
            Err(SemanticFragmentError::Invalid(format!(
                "record {key} is not an integer or string"
            )))
        },
        |value| Ok(RecordKey::Text(value.to_owned())),
    )
}

fn keyed_records<'a>(
    records: &'a [Value],
    key: &str,
) -> Result<BTreeMap<RecordKey, &'a Value>, SemanticFragmentError> {
    let mut keyed = BTreeMap::new();
    for record in records {
        let identity = record_key(record, key)?;
        if keyed.insert(identity.clone(), record).is_some() {
            return Err(SemanticFragmentError::Invalid(format!(
                "duplicate {key} {identity:?}"
            )));
        }
    }
    Ok(keyed)
}

fn merge_records(
    target: &mut Vec<Value>,
    additions: &[Value],
    key: &str,
) -> Result<(), SemanticFragmentError> {
    let mut identities = keyed_records(target, key)?
        .into_iter()
        .map(|(identity, value)| (identity, value.clone()))
        .collect::<BTreeMap<_, _>>();
    for addition in additions {
        let identity = record_key(addition, key)?;
        match identities.get(&identity) {
            Some(existing) if existing != addition => {
                return Err(SemanticFragmentError::Invalid(format!(
                    "fragment conflicts with frozen {key} {identity:?}"
                )));
            }
            Some(_) => {}
            None => {
                identities.insert(identity, addition.clone());
                target.push(addition.clone());
            }
        }
    }
    Ok(())
}

fn sort_records(records: &mut [Value], key: &str) -> Result<(), SemanticFragmentError> {
    let mut failure = None;
    records.sort_by_cached_key(|record| match record_key(record, key) {
        Ok(identity) => identity,
        Err(error) => {
            failure = Some(error);
            RecordKey::Text(String::new())
        }
    });
    failure.map_or(Ok(()), Err)
}

fn merge_enum_domain(
    domains: &mut Vec<Value>,
    addition: &Value,
) -> Result<(), SemanticFragmentError> {
    let domain = scalar_str(addition, "domain")?;
    let Some(existing) = domains
        .iter_mut()
        .find(|record| record.get("domain").and_then(Value::as_str) == Some(domain))
    else {
        let mut addition = addition.clone();
        sort_records(array_mut(&mut addition, "values")?, "code")?;
        domains.push(addition);
        return Ok(());
    };
    if existing.get("width_bits") != addition.get("width_bits") {
        return Err(SemanticFragmentError::Invalid(format!(
            "fragment conflicts with enum width for {domain}"
        )));
    }
    for (field, value) in addition.as_object().ok_or_else(|| {
        SemanticFragmentError::Invalid(format!("enum addition {domain} is not an object"))
    })? {
        if matches!(field.as_str(), "domain" | "width_bits" | "values") {
            continue;
        }
        if existing.get(field) != Some(value) {
            return Err(SemanticFragmentError::Invalid(format!(
                "fragment conflicts with enum {domain} field {field}"
            )));
        }
    }
    merge_records(
        array_mut(existing, "values")?,
        array(addition, "values")?,
        "code",
    )?;
    sort_records(array_mut(existing, "values")?, "code")
}

fn scalar_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, SemanticFragmentError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| SemanticFragmentError::Invalid(format!("record {field} is not a string")))
}

fn scalar_u64(value: &Value, field: &str) -> Result<u64, SemanticFragmentError> {
    value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        SemanticFragmentError::Invalid(format!("record {field} is not an unsigned integer"))
    })
}

fn scalar_i16(value: &Value, field: &str) -> Result<i16, SemanticFragmentError> {
    let raw = value.get(field).and_then(Value::as_i64).ok_or_else(|| {
        SemanticFragmentError::Invalid(format!("record {field} is not an integer"))
    })?;
    i16::try_from(raw)
        .map_err(|_| SemanticFragmentError::Invalid(format!("record {field} does not fit i16")))
}

fn record_field_names(record: &Value) -> Result<BTreeSet<&str>, SemanticFragmentError> {
    Ok(array(record, "fields")
        .or_else(|_| array(record, "columns"))?
        .iter()
        .map(|field| scalar_str(field, "name"))
        .collect::<Result<_, _>>()?)
}

#[derive(Debug, Error)]
pub enum SemanticFragmentError {
    #[error("semantic fragment is invalid: {0}")]
    Invalid(String),
    #[error(transparent)]
    Repository(#[from] super::repository_model::RepositoryModelError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    CanonicalJson(#[from] codefabric::contracts::jcs::CanonicalJsonError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    fn json_authority(path: &str) -> Value {
        let bytes = read_stable(&root().join(path), MAX_FRAGMENT_BYTES).unwrap();
        if path.ends_with(".yaml") {
            let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_slice(&bytes).unwrap();
            serde_json::to_value(yaml).unwrap()
        } else {
            serde_json::from_slice(&bytes).unwrap()
        }
    }

    #[test]
    fn current_fragments_compose_deterministically() {
        let fragments = SemanticFragmentSet::load(root()).unwrap();
        assert_eq!(
            fragments.render_json().unwrap(),
            fragments.render_json().unwrap()
        );
        assert_eq!(fragments.render_rust(), fragments.render_rust());
        assert!(fragments.render_rust().contains("INGEST_PYREFLY_MODULE_V1"));
        assert!(
            fragments
                .render_rust()
                .contains("INGEST_RUSTC_MIR_OWNER_V1")
        );
    }

    #[test]
    fn duplicate_lane_ownership_is_rejected() {
        let mut fragments = SemanticFragmentSet::load(root()).unwrap();
        fragments.documents[1].model.owned_table_codes.push(180);
        assert!(
            fragments
                .validate()
                .unwrap_err()
                .to_string()
                .contains("multiple lane owners")
        );
    }

    #[test]
    fn conflicting_frozen_schema_addition_is_rejected() {
        let mut fragments = SemanticFragmentSet::load(root()).unwrap();
        let mut schema = json_authority("contracts/schema/schema-contract-ir.json");
        let mut conflicting = schema["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["table_code"] == 180)
            .unwrap()
            .clone();
        conflicting["name"] = json!("wrong_type_detail");
        fragments.documents[0]
            .model
            .table_additions
            .push(conflicting);
        let error = fragments.compose_schema(&mut schema).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("conflicts with frozen table_code")
        );
    }

    #[test]
    fn registry_additions_are_composed_and_sorted_by_numeric_code() {
        let mut fragments = SemanticFragmentSet::load(root()).unwrap();
        let shared = &mut fragments.documents[0];
        shared.registry.property_codes.push(9);
        shared
            .registry
            .property_record_additions
            .push(json!({"property_code": 9, "marker": "fragment-only"}));
        shared.registry.enum_values.push(EnumValueReference {
            domain: "FRAGMENT_TEST".to_owned(),
            codes: vec![20, 3],
        });
        shared.registry.enum_domain_additions.push(json!({
            "domain": "FRAGMENT_TEST",
            "width_bits": 16,
            "values": [
                {"code": 20, "name": "TWENTY", "slug": "twenty", "meaning": "twenty"},
                {"code": 3, "name": "THREE", "slug": "three", "meaning": "three"}
            ]
        }));
        fragments.validate().unwrap();

        let mut properties = json_authority("contracts/registry/ontology-property-registry.yaml");
        let mut enums = json_authority("contracts/registry/enum-registry.yaml");
        fragments
            .compose_registries(&mut properties, &mut enums)
            .unwrap();
        let property_codes = properties["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| record["property_code"].as_u64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(property_codes[0], 9);
        let domain = enums["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|record| record["domain"] == "FRAGMENT_TEST")
            .unwrap();
        assert_eq!(domain["values"][0]["code"], 3);
        assert_eq!(domain["values"][1]["code"], 20);
    }

    #[test]
    fn absent_registry_reference_is_rejected() {
        let mut fragments = SemanticFragmentSet::load(root()).unwrap();
        fragments.documents[0].registry.property_codes.push(65_535);
        let properties = json_authority("contracts/registry/ontology-property-registry.yaml");
        let enums = json_authority("contracts/registry/enum-registry.yaml");
        let error = fragments
            .validate_registry_values(&properties, &enums)
            .unwrap_err();
        assert!(error.to_string().contains("absent property code 65535"));
    }
}
