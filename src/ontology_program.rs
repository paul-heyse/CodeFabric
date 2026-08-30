//! Reproducible Arrow package for the authored ontology program.
//!
//! The package is a generated projection over the schema/phrase authorities. It is
//! publication-neutral: candidate manifests may bind its identities, but Delta versions and
//! activation state never enter the logical or package identity domains.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Cursor;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use arrow_array::{
    Array as _, BinaryArray, BooleanArray, Int16Array, Int32Array, RecordBatch, StringArray,
};
use arrow_ipc::reader::StreamReader;
use arrow_schema::{ArrowError, SchemaRef};
use serde::Deserialize;
use thiserror::Error;

mod generated_bundle {
    include!("generated/ontology_program_bundle.rs");
}

/// Stable executable epoch-package format. V2 adds retained result/public/wire authorities.
pub const ONTOLOGY_PROGRAM_PACKAGE_VERSION: &str = "ontology-program-package.v2";

pub(crate) const RESULT_CONTRACT_SET_ARTIFACT: &str = "result-contract-set.json";
pub(crate) const TABLE_CONTRACT_SET_ARTIFACT: &str = "table-contract-set.json";
pub(crate) const SCHEMA_GENERATION_ARTIFACT: &str = "schema-generation.json";
pub(crate) const PUBLIC_SCHEMA_SET_ARTIFACT: &str = "public-schema-set.json";
pub(crate) const PUBLIC_WIRE_ARTIFACT: &str = "public-wire.pb";
pub(crate) const EXTENSION_REGISTRY_ARTIFACT: &str = "extension-registry.json";
pub(crate) const CHECKSUM_CONTRACT_ARTIFACT: &str = "checksum-contract.json";
const REQUIRED_RUNTIME_ARTIFACTS: [&str; 7] = [
    RESULT_CONTRACT_SET_ARTIFACT,
    TABLE_CONTRACT_SET_ARTIFACT,
    SCHEMA_GENERATION_ARTIFACT,
    PUBLIC_SCHEMA_SET_ARTIFACT,
    PUBLIC_WIRE_ARTIFACT,
    EXTENSION_REGISTRY_ARTIFACT,
    CHECKSUM_CONTRACT_ARTIFACT,
];

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
            // The normalized expression-edge relation is intentionally dense; 4K keeps every
            // current homogeneous relation in one canonical batch while retaining a hard bound.
            max_rows_per_batch: 4_096,
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
    pub runtime_artifact_identities: BTreeMap<String, String>,
    pub package_identity: String,
}

/// Digest-checked package handle admitted by the compiler/session boundary.
#[derive(Clone, Debug)]
pub struct OntologyProgramPackage {
    pub manifest: OntologyProgramManifest,
    pub members: BTreeMap<String, OntologyProgramMember>,
    pub runtime_artifacts: BTreeMap<String, Vec<u8>>,
}

/// Fully decoded runtime authority retained by an executable serving epoch.
#[derive(Clone, Debug)]
pub(crate) struct EpochRuntimeAuthority {
    result_schemas: crate::schema_registry::EpochResultSchemaRegistry,
    table_specs: crate::schema_registry::EpochTableSpecRegistry,
    extension_registry_bytes: Vec<u8>,
    checksum_version: String,
    identity: String,
    public_wire_file_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicSchemaSetArtifact {
    artifact_version: String,
    schemas: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChecksumContractArtifact {
    artifact_version: String,
    selected_version: String,
}

/// Durable content-addressed installation of one validated ontology program package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledOntologyProgramPackage {
    root: PathBuf,
    manifest_path: PathBuf,
    package_identity: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageArtifactMember {
    relation_id: String,
    file: String,
    member_identity: String,
    byte_length: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageRuntimeArtifact {
    artifact_id: String,
    file: String,
    artifact_identity: String,
    byte_length: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageArtifactManifest {
    package_version: String,
    bootstrap_schema_identity: String,
    authored_content_identity: String,
    logical_program_identity: String,
    packaging_profile_id: String,
    member_identities: BTreeMap<String, String>,
    runtime_artifact_identities: BTreeMap<String, String>,
    package_identity: String,
    members: Vec<PackageArtifactMember>,
    runtime_artifacts: Vec<PackageRuntimeArtifact>,
}

impl InstalledOntologyProgramPackage {
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    #[must_use]
    pub fn package_identity(&self) -> &str {
        &self.package_identity
    }

    /// Resolve one validated relation member without accepting caller-chosen paths.
    #[must_use]
    pub fn member_path(&self, relation_id: &str) -> PathBuf {
        self.root.join(format!("{relation_id}.arrow"))
    }
}

/// Publication-neutral seam copied into an external candidate manifest after publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateProgramBinding {
    pub package_identity: String,
    pub logical_program_identity: String,
    pub member_identities: BTreeMap<String, String>,
}

/// Application-owned authority DTO decoded from the Arrow program artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramAuthority {
    pub authority_id: String,
    pub authority_version: String,
    pub canonical_digest: String,
    pub canonical_source_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramEnumValue {
    pub domain: String,
    pub code: i32,
    pub name: String,
    pub authority: ProgramAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramEntityKind {
    pub code: i32,
    pub name: String,
    pub family_code: i16,
    pub language_applicability: String,
    pub query_visible: bool,
    pub authority: ProgramAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramRelationKind {
    pub code: i32,
    pub name: String,
    pub family_code: i16,
    pub family_name: String,
    pub cardinality: String,
    pub symmetric: bool,
    pub transitive: bool,
    pub self_edge_policy: String,
    pub owner_selection_rule: String,
    pub query_visible: bool,
    pub authority: ProgramAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramPropertyKind {
    pub code: i32,
    pub name: String,
    pub value_kind_code: i16,
    pub cardinality: String,
    pub storage_mapping: String,
    pub authority: ProgramAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramFactKind {
    pub code: i16,
    pub name: String,
    pub fact_form: String,
    pub authority: ProgramAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramProviderRawKind {
    pub provider_code: i16,
    pub raw_catalog_id: String,
    pub raw_namespace: String,
    pub raw_kind_code: i32,
    pub raw_name: String,
    pub normalized_kind_code: Option<i32>,
    pub authority: ProgramAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramOntologyEdge {
    pub subject_term_id: String,
    pub predicate_term_id: String,
    pub object_term_id: String,
    pub ordinal: i32,
    pub authority: ProgramAuthority,
}

/// Typed runtime view decoded from the digest-checked Arrow bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OntologyProgramVocabulary {
    pub enum_values: Vec<ProgramEnumValue>,
    pub entity_kinds: Vec<ProgramEntityKind>,
    pub relation_kinds: Vec<ProgramRelationKind>,
    pub property_kinds: Vec<ProgramPropertyKind>,
    pub fact_kinds: Vec<ProgramFactKind>,
    pub provider_raw_kinds: Vec<ProgramProviderRawKind>,
    pub phrase_authority: ProgramAuthority,
    pub query_form_authority: ProgramAuthority,
    pub edges: Vec<ProgramOntologyEdge>,
}

fn manifest_for_members(
    profile_id: &str,
    members: &BTreeMap<String, OntologyProgramMember>,
    runtime_artifacts: &BTreeMap<String, Vec<u8>>,
) -> OntologyProgramManifest {
    let bootstrap_schema_identity = framed(
        members
            .values()
            .map(|member| format!("{}:{:?}", member.relation_id, member.schema))
            .map(String::into_bytes),
    );
    let authored_content_identity = framed([
        generated_bundle::ONTOLOGY_PROGRAM_SOURCE_IDENTITY.as_bytes(),
        generated_bundle::ONTOLOGY_PROGRAM_PHRASE_AUTHORITY_IDENTITY.as_bytes(),
        generated_bundle::ONTOLOGY_PROGRAM_QUERY_FORM_AUTHORITY_IDENTITY.as_bytes(),
    ]);
    let logical_program_identity = framed(
        std::iter::once(bootstrap_schema_identity.as_bytes().to_vec())
            .chain(std::iter::once(
                authored_content_identity.as_bytes().to_vec(),
            ))
            .chain(members.values().map(logical_rows)),
    );
    let member_identities = members
        .iter()
        .map(|(name, member)| (name.clone(), member.member_identity.clone()))
        .collect::<BTreeMap<_, _>>();
    let runtime_artifact_identities = runtime_artifacts
        .iter()
        .map(|(name, bytes)| (name.clone(), framed([name.as_bytes(), bytes.as_slice()])))
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
        )
        .chain(
            runtime_artifact_identities
                .iter()
                .map(|(name, digest)| format!("runtime:{name}:{digest}").into_bytes()),
        ),
    );
    OntologyProgramManifest {
        package_version: ONTOLOGY_PROGRAM_PACKAGE_VERSION.into(),
        bootstrap_schema_identity,
        authored_content_identity,
        logical_program_identity,
        packaging_profile_id: profile_id.to_owned(),
        member_identities,
        runtime_artifact_identities,
        package_identity,
    }
}

fn canonical_public_schema_set() -> Result<Vec<u8>, OntologyProgramError> {
    let schemas = [
        (
            "cpg-semantic-query-request.schema.json",
            include_bytes!("../contracts/schema/cpg-semantic-query-request.schema.json").as_slice(),
        ),
        (
            "cpg-semantic-query-response.schema.json",
            include_bytes!("../contracts/schema/cpg-semantic-query-response.schema.json")
                .as_slice(),
        ),
        (
            "public-snapshot-metadata.schema.json",
            include_bytes!("../contracts/schema/public-snapshot-metadata.schema.json").as_slice(),
        ),
        (
            "public-status.schema.json",
            include_bytes!("../contracts/schema/public-status.schema.json").as_slice(),
        ),
        (
            "source-context.schema.json",
            include_bytes!("../contracts/schema/source-context.schema.json").as_slice(),
        ),
    ];
    let values = schemas
        .into_iter()
        .map(|(name, bytes)| {
            serde_json::from_slice::<serde_json::Value>(bytes)
                .map(|value| (name, value))
                .map_err(|error| {
                    OntologyProgramError::Artifact(format!(
                        "public schema {name} is not JSON: {error}"
                    ))
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    crate::contracts::jcs::canonicalize_value(&serde_json::json!({
        "artifact_version": "epoch-public-schema-set.v1",
        "schemas": values,
    }))
    .map_err(|error| OntologyProgramError::Artifact(error.to_string()))
}

fn build_runtime_artifacts(
    checksum_version: &str,
) -> Result<BTreeMap<String, Vec<u8>>, OntologyProgramError> {
    let checksum_contract = crate::contracts::jcs::canonicalize_value(&serde_json::json!({
        "artifact_version": "epoch-result-checksum-contract.v1",
        "selected_version": checksum_version,
    }))
    .map_err(|error| OntologyProgramError::Artifact(error.to_string()))?;
    Ok(BTreeMap::from([
        (
            RESULT_CONTRACT_SET_ARTIFACT.into(),
            crate::schema_registry::epoch_result_contract_set_bytes()?,
        ),
        (
            TABLE_CONTRACT_SET_ARTIFACT.into(),
            crate::schema_registry::epoch_table_contract_set_bytes()?,
        ),
        (
            SCHEMA_GENERATION_ARTIFACT.into(),
            include_bytes!("../contracts/schema/schema-contract-ir.json").to_vec(),
        ),
        (
            PUBLIC_SCHEMA_SET_ARTIFACT.into(),
            canonical_public_schema_set()?,
        ),
        (
            PUBLIC_WIRE_ARTIFACT.into(),
            include_bytes!("../tooling/proto/production-descriptor.pb").to_vec(),
        ),
        (
            EXTENSION_REGISTRY_ARTIFACT.into(),
            crate::schema_registry::epoch_extension_registry_bytes()?,
        ),
        (CHECKSUM_CONTRACT_ARTIFACT.into(), checksum_contract),
    ]))
}

fn protobuf_file_descriptor_count(bytes: &[u8]) -> Result<usize, OntologyProgramError> {
    fn varint(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
        let mut value = 0_u64;
        for shift in (0..=63).step_by(7) {
            let byte = *bytes.get(*cursor)?;
            *cursor += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }
    let mut cursor = 0;
    let mut files = 0;
    while cursor < bytes.len() {
        let key = varint(bytes, &mut cursor).ok_or_else(|| {
            OntologyProgramError::Artifact("public wire descriptor has an invalid tag".into())
        })?;
        let field = key >> 3;
        match key & 7 {
            0 => {
                varint(bytes, &mut cursor).ok_or_else(|| {
                    OntologyProgramError::Artifact(
                        "public wire descriptor has an invalid varint".into(),
                    )
                })?;
            }
            1 => {
                cursor = cursor.checked_add(8).ok_or_else(|| {
                    OntologyProgramError::Artifact("public wire descriptor offset overflow".into())
                })?;
            }
            2 => {
                let length = usize::try_from(varint(bytes, &mut cursor).ok_or_else(|| {
                    OntologyProgramError::Artifact(
                        "public wire descriptor has an invalid length".into(),
                    )
                })?)
                .map_err(|_| {
                    OntologyProgramError::Artifact(
                        "public wire descriptor length exceeds the platform".into(),
                    )
                })?;
                cursor = cursor.checked_add(length).ok_or_else(|| {
                    OntologyProgramError::Artifact("public wire descriptor offset overflow".into())
                })?;
                if field == 1 {
                    files += 1;
                }
            }
            5 => {
                cursor = cursor.checked_add(4).ok_or_else(|| {
                    OntologyProgramError::Artifact("public wire descriptor offset overflow".into())
                })?;
            }
            _ => {
                return Err(OntologyProgramError::Artifact(
                    "public wire descriptor uses an unsupported protobuf wire type".into(),
                ));
            }
        }
        if cursor > bytes.len() {
            return Err(OntologyProgramError::Artifact(
                "public wire descriptor is truncated".into(),
            ));
        }
    }
    if files == 0 {
        return Err(OntologyProgramError::Artifact(
            "public wire descriptor contains no files".into(),
        ));
    }
    Ok(files)
}

impl EpochRuntimeAuthority {
    /// Decode every runtime authority from retained package bytes, never generated globals.
    pub(crate) fn decode(package: &OntologyProgramPackage) -> Result<Self, OntologyProgramError> {
        let artifact = |name: &'static str| {
            package.runtime_artifacts.get(name).ok_or_else(|| {
                OntologyProgramError::Artifact(format!(
                    "retained runtime artifact {name} is absent"
                ))
            })
        };
        let result_schemas = crate::schema_registry::EpochResultSchemaRegistry::decode(artifact(
            RESULT_CONTRACT_SET_ARTIFACT,
        )?)?;
        let table_specs = crate::schema_registry::EpochTableSpecRegistry::decode(artifact(
            TABLE_CONTRACT_SET_ARTIFACT,
        )?)?;
        let extension_registry_bytes = artifact(EXTENSION_REGISTRY_ARTIFACT)?.clone();
        crate::schema_registry::epoch_extension_type_registrations(&extension_registry_bytes)?;
        let schema_generation: serde_json::Value =
            serde_json::from_slice(artifact(SCHEMA_GENERATION_ARTIFACT)?).map_err(|error| {
                OntologyProgramError::Artifact(format!(
                    "retained schema-generation IR is invalid: {error}"
                ))
            })?;
        if schema_generation.get("canonical_digest").is_none() {
            return Err(OntologyProgramError::Artifact(
                "retained schema-generation IR has no source identity".into(),
            ));
        }
        let public_schemas: PublicSchemaSetArtifact =
            serde_json::from_slice(artifact(PUBLIC_SCHEMA_SET_ARTIFACT)?).map_err(|error| {
                OntologyProgramError::Artifact(format!(
                    "retained public schemas are invalid: {error}"
                ))
            })?;
        if public_schemas.artifact_version != "epoch-public-schema-set.v1"
            || !public_schemas
                .schemas
                .contains_key("cpg-semantic-query-response.schema.json")
            || public_schemas.schemas.values().any(|schema| {
                schema
                    .get("$schema")
                    .and_then(serde_json::Value::as_str)
                    .is_none()
                    || schema
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .is_none()
            })
        {
            return Err(OntologyProgramError::Artifact(
                "retained public schema set is incomplete".into(),
            ));
        }
        let public_wire_file_count =
            protobuf_file_descriptor_count(artifact(PUBLIC_WIRE_ARTIFACT)?)?;
        let checksum: ChecksumContractArtifact =
            serde_json::from_slice(artifact(CHECKSUM_CONTRACT_ARTIFACT)?).map_err(|error| {
                OntologyProgramError::Artifact(format!(
                    "retained checksum contract is invalid: {error}"
                ))
            })?;
        if checksum.artifact_version != "epoch-result-checksum-contract.v1"
            || checksum.selected_version != result_checksum_version(package)?
        {
            return Err(OntologyProgramError::Artifact(
                "retained checksum contract differs from the executable program".into(),
            ));
        }
        let identity = framed(
            package
                .manifest
                .runtime_artifact_identities
                .iter()
                .map(|(name, digest)| format!("{name}:{digest}").into_bytes()),
        );
        Ok(Self {
            result_schemas,
            table_specs,
            extension_registry_bytes,
            checksum_version: checksum.selected_version,
            identity,
            public_wire_file_count,
        })
    }

    pub(crate) fn extension_type_registrations(
        &self,
    ) -> Result<Vec<datafusion::logical_expr::registry::ExtensionTypeRegistrationRef>, ArrowError>
    {
        crate::schema_registry::epoch_extension_type_registrations(&self.extension_registry_bytes)
    }

    #[must_use]
    pub(crate) fn result_schema(&self, identity: &str) -> Option<SchemaRef> {
        self.result_schemas.result_schema(identity)
    }

    #[must_use]
    pub(crate) fn project_result_schema(
        &self,
        identity: &str,
        names: &[&str],
    ) -> Option<SchemaRef> {
        self.result_schemas.project_result_schema(identity, names)
    }

    #[must_use]
    pub(crate) const fn table_specs(&self) -> &crate::schema_registry::EpochTableSpecRegistry {
        &self.table_specs
    }

    #[must_use]
    pub(crate) fn checksum_version(&self) -> &str {
        &self.checksum_version
    }

    #[must_use]
    pub(crate) fn identity(&self) -> &str {
        &self.identity
    }

    #[must_use]
    pub(crate) const fn public_wire_file_count(&self) -> usize {
        self.public_wire_file_count
    }
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
    #[error("ONTOLOGY_PROGRAM_ARTIFACT_INVALID:{0}")]
    Artifact(String),
    #[error("ontology program artifact I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn artifact_io(path: &Path, source: std::io::Error) -> OntologyProgramError {
    OntologyProgramError::Io {
        path: path.to_owned(),
        source,
    }
}

fn package_artifact_manifest(
    package: &OntologyProgramPackage,
) -> Result<Vec<u8>, OntologyProgramError> {
    let members = package
        .members
        .values()
        .map(|member| {
            serde_json::json!({
                "relation_id": member.relation_id,
                "file": format!("{}.arrow", member.relation_id),
                "member_identity": member.member_identity,
                "byte_length": member.ipc_bytes.len(),
            })
        })
        .collect::<Vec<_>>();
    let runtime_artifacts = package
        .runtime_artifacts
        .iter()
        .map(|(artifact_id, bytes)| {
            serde_json::json!({
                "artifact_id": artifact_id,
                "file": artifact_id,
                "artifact_identity": package.manifest.runtime_artifact_identities[artifact_id],
                "byte_length": bytes.len(),
            })
        })
        .collect::<Vec<_>>();
    crate::contracts::jcs::canonicalize_value(&serde_json::json!({
        "package_version": package.manifest.package_version,
        "bootstrap_schema_identity": package.manifest.bootstrap_schema_identity,
        "authored_content_identity": package.manifest.authored_content_identity,
        "logical_program_identity": package.manifest.logical_program_identity,
        "packaging_profile_id": package.manifest.packaging_profile_id,
        "member_identities": package.manifest.member_identities,
        "runtime_artifact_identities": package.manifest.runtime_artifact_identities,
        "package_identity": package.manifest.package_identity,
        "members": members,
        "runtime_artifacts": runtime_artifacts,
    }))
    .map_err(|error| OntologyProgramError::Artifact(error.to_string()))
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), OntologyProgramError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|source| artifact_io(path, source))?;
    std::io::Write::write_all(&mut file, bytes).map_err(|source| artifact_io(path, source))?;
    file.sync_all().map_err(|source| artifact_io(path, source))
}

fn sync_artifact_directory(path: &Path) -> Result<(), OntologyProgramError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| artifact_io(path, source))
}

/// Verify a durable package installation against the typed, digest-checked in-memory package.
///
/// # Errors
///
/// Rejects a path/identity mismatch, missing or changed manifest/member bytes, an unsafe relation
/// identifier, or an invalid in-memory package.
pub fn verify_installed_ontology_program_package(
    installation: &InstalledOntologyProgramPackage,
    package: &OntologyProgramPackage,
) -> Result<(), OntologyProgramError> {
    validate_ontology_program_package(package)?;
    if installation.package_identity != package.manifest.package_identity
        || installation.manifest_path != installation.root.join("manifest.json")
    {
        return Err(OntologyProgramError::Artifact(
            "installation identity or manifest address differs".into(),
        ));
    }
    let expected_manifest = package_artifact_manifest(package)?;
    let actual_manifest = fs::read(&installation.manifest_path)
        .map_err(|source| artifact_io(&installation.manifest_path, source))?;
    if actual_manifest != expected_manifest {
        return Err(OntologyProgramError::Artifact(
            "installed package manifest bytes differ".into(),
        ));
    }
    for (relation_id, member) in &package.members {
        if relation_id.is_empty()
            || !relation_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(OntologyProgramError::Artifact(format!(
                "unsafe relation identifier {relation_id:?}"
            )));
        }
        let path = installation.member_path(relation_id);
        let bytes = fs::read(&path).map_err(|source| artifact_io(&path, source))?;
        if bytes != member.ipc_bytes
            || framed([relation_id.as_bytes(), bytes.as_slice()]) != member.member_identity
        {
            return Err(OntologyProgramError::Artifact(format!(
                "installed member {relation_id} differs"
            )));
        }
    }
    for (artifact_id, bytes) in &package.runtime_artifacts {
        let path = installation.root.join(artifact_id);
        let installed = fs::read(&path).map_err(|source| artifact_io(&path, source))?;
        let expected_identity = framed([artifact_id.as_bytes(), installed.as_slice()]);
        if installed != *bytes
            || package
                .manifest
                .runtime_artifact_identities
                .get(artifact_id)
                != Some(&expected_identity)
        {
            return Err(OntologyProgramError::Artifact(format!(
                "installed runtime artifact {artifact_id} differs"
            )));
        }
    }
    Ok(())
}

/// Atomically install one package at a content-addressed durable directory and verify readback.
///
/// Identical retries are read-only no-ops. The address is derived exclusively from the validated
/// package identity; callers cannot choose member filenames or overwrite another package.
///
/// # Errors
///
/// Rejects an invalid package, unsafe package identity, conflicting existing bytes, or I/O error.
pub fn install_ontology_program_package(
    state_root: &Path,
    package: &OntologyProgramPackage,
) -> Result<InstalledOntologyProgramPackage, OntologyProgramError> {
    validate_ontology_program_package(package)?;
    let Some(identity) = package.manifest.package_identity.strip_prefix("b3:") else {
        return Err(OntologyProgramError::Artifact(
            "package identity is not a b3 digest".into(),
        ));
    };
    if identity.len() != 64 || !identity.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(OntologyProgramError::Artifact(
            "package identity has invalid digest bytes".into(),
        ));
    }
    let packages_root = state_root.join("ontology-programs");
    fs::create_dir_all(&packages_root).map_err(|source| artifact_io(&packages_root, source))?;
    fs::set_permissions(&packages_root, fs::Permissions::from_mode(0o700))
        .map_err(|source| artifact_io(&packages_root, source))?;
    let final_root = packages_root.join(identity);
    let installation = InstalledOntologyProgramPackage {
        manifest_path: final_root.join("manifest.json"),
        root: final_root.clone(),
        package_identity: package.manifest.package_identity.clone(),
    };
    if final_root.exists() {
        verify_installed_ontology_program_package(&installation, package)?;
        return Ok(installation);
    }

    let stage = (0_u8..32)
        .map(|attempt| {
            packages_root.join(format!(
                ".{identity}.{}.{}.tmp",
                std::process::id(),
                attempt
            ))
        })
        .find(|path| fs::create_dir(path).is_ok())
        .ok_or_else(|| OntologyProgramError::Artifact("cannot allocate package stage".into()))?;
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700))
        .map_err(|source| artifact_io(&stage, source))?;
    let staged = InstalledOntologyProgramPackage {
        manifest_path: stage.join("manifest.json"),
        root: stage.clone(),
        package_identity: package.manifest.package_identity.clone(),
    };
    let result = (|| {
        for (relation_id, member) in &package.members {
            if relation_id.is_empty()
                || !relation_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            {
                return Err(OntologyProgramError::Artifact(format!(
                    "unsafe relation identifier {relation_id:?}"
                )));
            }
            write_private_file(&staged.member_path(relation_id), &member.ipc_bytes)?;
        }
        for (artifact_id, bytes) in &package.runtime_artifacts {
            write_private_file(&staged.root.join(artifact_id), bytes)?;
        }
        write_private_file(&staged.manifest_path, &package_artifact_manifest(package)?)?;
        sync_artifact_directory(&stage)?;
        fs::rename(&stage, &final_root).map_err(|source| artifact_io(&final_root, source))?;
        sync_artifact_directory(&packages_root)?;
        verify_installed_ontology_program_package(&installation, package)
    })();
    if result.is_err() && stage.exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    result?;
    Ok(installation)
}

/// Resolve and authenticate a previously installed content-addressed package without consulting
/// the current generated bundle.
///
/// # Errors
///
/// Rejects an unsafe identity, missing or malformed artifact, non-canonical member address,
/// length/digest mismatch, or invalid reconstructed package.
pub fn load_installed_ontology_program_package(
    state_root: &Path,
    package_identity: &str,
) -> Result<OntologyProgramPackage, OntologyProgramError> {
    let Some(identity) = package_identity.strip_prefix("b3:") else {
        return Err(OntologyProgramError::Artifact(
            "package identity is not a b3 digest".into(),
        ));
    };
    if identity.len() != 64 || !identity.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(OntologyProgramError::Artifact(
            "package identity has invalid digest bytes".into(),
        ));
    }
    let root = state_root.join("ontology-programs").join(identity);
    let manifest_path = root.join("manifest.json");
    let manifest_bytes =
        fs::read(&manifest_path).map_err(|source| artifact_io(&manifest_path, source))?;
    let artifact: PackageArtifactManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|error| {
            OntologyProgramError::Artifact(format!("invalid package manifest: {error}"))
        })?;
    if artifact.package_identity != package_identity
        || artifact.package_version != ONTOLOGY_PROGRAM_PACKAGE_VERSION
    {
        return Err(OntologyProgramError::Artifact(
            "installed package manifest identity or version differs".into(),
        ));
    }
    let mut members = BTreeMap::new();
    for member in artifact.members {
        if member.relation_id.is_empty()
            || !member
                .relation_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            || member.file != format!("{}.arrow", member.relation_id)
        {
            return Err(OntologyProgramError::Artifact(
                "installed package contains an unsafe member address".into(),
            ));
        }
        let path = root.join(&member.file);
        let bytes = fs::read(&path).map_err(|source| artifact_io(&path, source))?;
        if bytes.len() != member.byte_length
            || framed([member.relation_id.as_bytes(), bytes.as_slice()]) != member.member_identity
            || artifact.member_identities.get(&member.relation_id) != Some(&member.member_identity)
        {
            return Err(OntologyProgramError::Artifact(format!(
                "installed member {} differs from its manifest",
                member.relation_id
            )));
        }
        let decoded = decode_member(&member.relation_id, &bytes)?;
        if members.insert(member.relation_id, decoded).is_some() {
            return Err(OntologyProgramError::Artifact(
                "installed package repeats a relation".into(),
            ));
        }
    }
    let mut runtime_artifacts = BTreeMap::new();
    for retained in artifact.runtime_artifacts {
        if !REQUIRED_RUNTIME_ARTIFACTS.contains(&retained.artifact_id.as_str())
            || retained.file != retained.artifact_id
            || artifact
                .runtime_artifact_identities
                .get(&retained.artifact_id)
                != Some(&retained.artifact_identity)
        {
            return Err(OntologyProgramError::Artifact(
                "installed package contains an unsafe runtime artifact address".into(),
            ));
        }
        let path = root.join(&retained.file);
        let bytes = fs::read(&path).map_err(|source| artifact_io(&path, source))?;
        if bytes.len() != retained.byte_length
            || framed([retained.artifact_id.as_bytes(), bytes.as_slice()])
                != retained.artifact_identity
            || runtime_artifacts
                .insert(retained.artifact_id, bytes)
                .is_some()
        {
            return Err(OntologyProgramError::Artifact(
                "installed runtime artifact differs from its manifest".into(),
            ));
        }
    }
    let package = OntologyProgramPackage {
        manifest: OntologyProgramManifest {
            package_version: artifact.package_version,
            bootstrap_schema_identity: artifact.bootstrap_schema_identity,
            authored_content_identity: artifact.authored_content_identity,
            logical_program_identity: artifact.logical_program_identity,
            packaging_profile_id: artifact.packaging_profile_id,
            member_identities: artifact.member_identities,
            runtime_artifact_identities: artifact.runtime_artifact_identities,
            package_identity: artifact.package_identity,
        },
        members,
        runtime_artifacts,
    };
    validate_ontology_program_package(&package)?;
    Ok(package)
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

fn decode_member(
    relation_id: &str,
    ipc_bytes: &[u8],
) -> Result<OntologyProgramMember, OntologyProgramError> {
    let reader = StreamReader::try_new(Cursor::new(ipc_bytes), None)?;
    let schema = reader.schema();
    let batches = reader.collect::<Result<Vec<_>, _>>()?;
    if batches.is_empty() || batches.iter().any(|batch| batch.schema() != schema) {
        return Err(OntologyProgramError::Contract(format!(
            "{relation_id} has an empty or heterogeneous artifact stream"
        )));
    }
    Ok(OntologyProgramMember {
        relation_id: relation_id.to_owned(),
        schema,
        batches,
        ipc_bytes: ipc_bytes.to_vec(),
        member_identity: framed([relation_id.as_bytes(), ipc_bytes]),
    })
}

fn generated_program_members(
    max_rows_per_batch: usize,
) -> Result<BTreeMap<String, OntologyProgramMember>, OntologyProgramError> {
    let mut outer = StreamReader::try_new(
        Cursor::new(generated_bundle::ONTOLOGY_PROGRAM_BUNDLE_IPC),
        None,
    )?;
    let Some(container) = outer.next().transpose()? else {
        return Err(OntologyProgramError::Artifact(
            "generated ontology-program bundle is empty".into(),
        ));
    };
    if outer.next().is_some() {
        return Err(OntologyProgramError::Artifact(
            "generated ontology-program bundle has multiple container batches".into(),
        ));
    }
    let relation_ids = container
        .column_by_name("relation_id")
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| OntologyProgramError::Artifact("bundle relation_id is not Utf8".into()))?;
    let streams = container
        .column_by_name("ipc_stream")
        .and_then(|column| column.as_any().downcast_ref::<BinaryArray>())
        .ok_or_else(|| OntologyProgramError::Artifact("bundle ipc_stream is not Binary".into()))?;
    let mut members = BTreeMap::new();
    for row in 0..container.num_rows() {
        let relation_id = relation_ids.value(row);
        let member = decode_member(relation_id, streams.value(row))?;
        if member
            .batches
            .iter()
            .any(|batch| batch.num_rows() > max_rows_per_batch)
        {
            return Err(OntologyProgramError::Resource(format!(
                "{relation_id} exceeds profile row limit {max_rows_per_batch}"
            )));
        }
        if members.insert(relation_id.to_owned(), member).is_some() {
            return Err(OntologyProgramError::Contract(format!(
                "duplicate generated relation {relation_id}"
            )));
        }
    }
    Ok(members)
}

pub(crate) fn program_batch<'a>(
    package: &'a OntologyProgramPackage,
    relation_id: &str,
) -> Result<&'a RecordBatch, OntologyProgramError> {
    let member = package
        .members
        .get(relation_id)
        .ok_or_else(|| OntologyProgramError::Contract(format!("missing {relation_id}")))?;
    if member.batches.len() != 1 {
        return Err(OntologyProgramError::Contract(format!(
            "{relation_id} is not one canonical batch"
        )));
    }
    Ok(&member.batches[0])
}

fn strings<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a StringArray, OntologyProgramError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| OntologyProgramError::Contract(format!("{name} is not Utf8")))
}

fn int16s<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Int16Array, OntologyProgramError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<Int16Array>())
        .ok_or_else(|| OntologyProgramError::Contract(format!("{name} is not Int16")))
}

fn int32s<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Int32Array, OntologyProgramError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<Int32Array>())
        .ok_or_else(|| OntologyProgramError::Contract(format!("{name} is not Int32")))
}

fn bools<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a BooleanArray, OntologyProgramError> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref::<BooleanArray>())
        .ok_or_else(|| OntologyProgramError::Contract(format!("{name} is not Boolean")))
}

fn decoded_authority(
    batch: &RecordBatch,
    row: usize,
) -> Result<ProgramAuthority, OntologyProgramError> {
    Ok(ProgramAuthority {
        authority_id: strings(batch, "authority_id")?.value(row).to_owned(),
        authority_version: strings(batch, "authority_version")?.value(row).to_owned(),
        canonical_digest: strings(batch, "canonical_digest")?.value(row).to_owned(),
        canonical_source_path: strings(batch, "canonical_source_path")?
            .value(row)
            .to_owned(),
    })
}

/// Decode the generated Arrow bundle into application DTOs used by ontology publication.
#[allow(clippy::too_many_lines)] // Keeps one auditable decoder for the generated relation family.
pub(crate) fn ontology_program_vocabulary()
-> Result<OntologyProgramVocabulary, OntologyProgramError> {
    let package = build_ontology_program_package(&OntologyPackagingProfile::default())?;
    let enum_batch = program_batch(&package, "program.enum_value")?;
    let enum_domains = strings(enum_batch, "domain")?;
    let enum_codes = int32s(enum_batch, "code")?;
    let enum_names = strings(enum_batch, "name")?;
    let enum_values = (0..enum_batch.num_rows())
        .map(|row| {
            Ok(ProgramEnumValue {
                domain: enum_domains.value(row).to_owned(),
                code: enum_codes.value(row),
                name: enum_names.value(row).to_owned(),
                authority: decoded_authority(enum_batch, row)?,
            })
        })
        .collect::<Result<Vec<_>, OntologyProgramError>>()?;

    let entity_batch = program_batch(&package, "program.entity_kind")?;
    let entity_codes = int32s(entity_batch, "code")?;
    let entity_names = strings(entity_batch, "name")?;
    let entity_families = int16s(entity_batch, "family_code")?;
    let entity_languages = strings(entity_batch, "language_applicability")?;
    let entity_visible = bools(entity_batch, "query_visible")?;
    let entity_kinds = (0..entity_batch.num_rows())
        .map(|row| {
            Ok(ProgramEntityKind {
                code: entity_codes.value(row),
                name: entity_names.value(row).to_owned(),
                family_code: entity_families.value(row),
                language_applicability: entity_languages.value(row).to_owned(),
                query_visible: entity_visible.value(row),
                authority: decoded_authority(entity_batch, row)?,
            })
        })
        .collect::<Result<Vec<_>, OntologyProgramError>>()?;

    let relation_batch = program_batch(&package, "program.relation_kind")?;
    let relation_codes = int32s(relation_batch, "code")?;
    let relation_names = strings(relation_batch, "name")?;
    let relation_families = int16s(relation_batch, "family_code")?;
    let relation_family_names = strings(relation_batch, "family_name")?;
    let relation_cardinality = strings(relation_batch, "cardinality")?;
    let relation_symmetric = bools(relation_batch, "symmetric")?;
    let relation_transitive = bools(relation_batch, "transitive")?;
    let relation_self_edges = strings(relation_batch, "self_edge_policy")?;
    let relation_owners = strings(relation_batch, "owner_selection_rule")?;
    let relation_visible = bools(relation_batch, "query_visible")?;
    let relation_kinds = (0..relation_batch.num_rows())
        .map(|row| {
            Ok(ProgramRelationKind {
                code: relation_codes.value(row),
                name: relation_names.value(row).to_owned(),
                family_code: relation_families.value(row),
                family_name: relation_family_names.value(row).to_owned(),
                cardinality: relation_cardinality.value(row).to_owned(),
                symmetric: relation_symmetric.value(row),
                transitive: relation_transitive.value(row),
                self_edge_policy: relation_self_edges.value(row).to_owned(),
                owner_selection_rule: relation_owners.value(row).to_owned(),
                query_visible: relation_visible.value(row),
                authority: decoded_authority(relation_batch, row)?,
            })
        })
        .collect::<Result<Vec<_>, OntologyProgramError>>()?;

    let property_batch = program_batch(&package, "program.property_kind")?;
    let property_codes = int32s(property_batch, "code")?;
    let property_names = strings(property_batch, "name")?;
    let property_value_kinds = int16s(property_batch, "value_kind_code")?;
    let property_cardinality = strings(property_batch, "cardinality")?;
    let property_storage = strings(property_batch, "storage_mapping")?;
    let property_kinds = (0..property_batch.num_rows())
        .map(|row| {
            Ok(ProgramPropertyKind {
                code: property_codes.value(row),
                name: property_names.value(row).to_owned(),
                value_kind_code: property_value_kinds.value(row),
                cardinality: property_cardinality.value(row).to_owned(),
                storage_mapping: property_storage.value(row).to_owned(),
                authority: decoded_authority(property_batch, row)?,
            })
        })
        .collect::<Result<Vec<_>, OntologyProgramError>>()?;

    let fact_batch = program_batch(&package, "program.fact_kind")?;
    let fact_codes = int16s(fact_batch, "code")?;
    let fact_names = strings(fact_batch, "name")?;
    let fact_forms = strings(fact_batch, "fact_form")?;
    let fact_kinds = (0..fact_batch.num_rows())
        .map(|row| {
            Ok(ProgramFactKind {
                code: fact_codes.value(row),
                name: fact_names.value(row).to_owned(),
                fact_form: fact_forms.value(row).to_owned(),
                authority: decoded_authority(fact_batch, row)?,
            })
        })
        .collect::<Result<Vec<_>, OntologyProgramError>>()?;

    let raw_batch = program_batch(&package, "program.provider_raw_kind")?;
    let raw_provider_codes = int16s(raw_batch, "provider_code")?;
    let raw_catalogs = strings(raw_batch, "raw_catalog_id")?;
    let raw_namespaces = strings(raw_batch, "raw_namespace")?;
    let raw_codes = int32s(raw_batch, "raw_kind_code")?;
    let raw_names = strings(raw_batch, "raw_name")?;
    let raw_normalized = int32s(raw_batch, "normalized_kind_code")?;
    let provider_raw_kinds = (0..raw_batch.num_rows())
        .map(|row| {
            Ok(ProgramProviderRawKind {
                provider_code: raw_provider_codes.value(row),
                raw_catalog_id: raw_catalogs.value(row).to_owned(),
                raw_namespace: raw_namespaces.value(row).to_owned(),
                raw_kind_code: raw_codes.value(row),
                raw_name: raw_names.value(row).to_owned(),
                normalized_kind_code: (!raw_normalized.is_null(row))
                    .then(|| raw_normalized.value(row)),
                authority: decoded_authority(raw_batch, row)?,
            })
        })
        .collect::<Result<Vec<_>, OntologyProgramError>>()?;

    let edge_batch = program_batch(&package, "program.ontology_edge")?;
    let edge_subjects = strings(edge_batch, "subject_term_id")?;
    let edge_predicates = strings(edge_batch, "predicate_term_id")?;
    let edge_objects = strings(edge_batch, "object_term_id")?;
    let edge_ordinals = int32s(edge_batch, "ordinal")?;
    let edges = (0..edge_batch.num_rows())
        .map(|row| {
            Ok(ProgramOntologyEdge {
                subject_term_id: edge_subjects.value(row).to_owned(),
                predicate_term_id: edge_predicates.value(row).to_owned(),
                object_term_id: edge_objects.value(row).to_owned(),
                ordinal: edge_ordinals.value(row),
                authority: decoded_authority(edge_batch, row)?,
            })
        })
        .collect::<Result<Vec<_>, OntologyProgramError>>()?;

    let phrase_authority = ProgramAuthority {
        authority_id: "codefabric.registry.phrase-registry".into(),
        authority_version: "1.0".into(),
        canonical_digest: generated_bundle::ONTOLOGY_PROGRAM_PHRASE_AUTHORITY_IDENTITY.into(),
        canonical_source_path: "contracts/registry/phrase-registry.yaml".into(),
    };
    let query_form_authority = ProgramAuthority {
        authority_id: "codefabric.query.form-contract".into(),
        authority_version: "1.0".into(),
        canonical_digest: generated_bundle::ONTOLOGY_PROGRAM_QUERY_FORM_AUTHORITY_IDENTITY.into(),
        canonical_source_path: "contracts/query/query-form-contract.json".into(),
    };
    Ok(OntologyProgramVocabulary {
        enum_values,
        entity_kinds,
        relation_kinds,
        property_kinds,
        fact_kinds,
        provider_raw_kinds,
        phrase_authority,
        query_form_authority,
        edges,
    })
}

/// Resolve the result checksum algorithm from the candidate's Arrow result bindings.
pub(crate) fn result_checksum_version(
    package: &OntologyProgramPackage,
) -> Result<String, OntologyProgramError> {
    validate_ontology_program_package(package)?;
    let batch = program_batch(package, "program.result_binding")?;
    let versions = strings(batch, "checksum_algorithm_version")?;
    let values = (0..batch.num_rows())
        .map(|row| versions.value(row))
        .collect::<BTreeSet<_>>();
    if values.len() != 1 {
        return Err(OntologyProgramError::Contract(
            "result bindings do not select exactly one checksum algorithm".into(),
        ));
    }
    Ok((*values.iter().next().expect("one value was counted")).to_owned())
}

fn logical_rows(member: &OntologyProgramMember) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(member.relation_id.as_bytes());
    for batch in &member.batches {
        bytes.extend_from_slice(format!("{batch:?}").as_bytes());
    }
    bytes
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
    let members = generated_program_members(profile.max_rows_per_batch)?;
    let result_bindings = members
        .get("program.result_binding")
        .ok_or_else(|| OntologyProgramError::Contract("program.result_binding is absent".into()))?;
    let versions = strings(&result_bindings.batches[0], "checksum_algorithm_version")?;
    let checksum_versions = (0..result_bindings.batches[0].num_rows())
        .map(|row| versions.value(row))
        .collect::<BTreeSet<_>>();
    if checksum_versions.len() != 1 {
        return Err(OntologyProgramError::Contract(
            "result bindings do not select exactly one checksum algorithm".into(),
        ));
    }
    let checksum_version = checksum_versions.iter().next().copied().ok_or_else(|| {
        OntologyProgramError::Contract("result bindings do not select a checksum algorithm".into())
    })?;
    let runtime_artifacts = build_runtime_artifacts(checksum_version)?;
    let manifest = manifest_for_members(&profile.profile_id, &members, &runtime_artifacts);
    let package = OntologyProgramPackage {
        manifest,
        members,
        runtime_artifacts,
    };
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
    if package
        .runtime_artifacts
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != REQUIRED_RUNTIME_ARTIFACTS.into_iter().collect()
        || package.runtime_artifacts.keys().collect::<BTreeSet<_>>()
            != package
                .manifest
                .runtime_artifact_identities
                .keys()
                .collect()
    {
        return Err(OntologyProgramError::Contract(
            "runtime authority artifact census differs from manifest".into(),
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
    for (name, bytes) in &package.runtime_artifacts {
        let expected = framed([name.as_bytes(), bytes.as_slice()]);
        if package.manifest.runtime_artifact_identities.get(name) != Some(&expected) {
            return Err(OntologyProgramError::Digest(name.clone()));
        }
    }
    crate::schema_registry::EpochResultSchemaRegistry::decode(
        &package.runtime_artifacts[RESULT_CONTRACT_SET_ARTIFACT],
    )?;
    crate::schema_registry::epoch_extension_type_registrations(
        &package.runtime_artifacts[EXTENSION_REGISTRY_ARTIFACT],
    )?;
    let expected = manifest_for_members(
        &package.manifest.packaging_profile_id,
        &package.members,
        &package.runtime_artifacts,
    );
    if package.manifest != expected {
        return Err(OntologyProgramError::Digest(
            "package manifest identity closure".into(),
        ));
    }
    Ok(())
}

/// Recompute the complete package identity after a trusted compiler changes program/runtime
/// members. This grants no activation authority; installation and owner acceptance remain
/// separate authenticated steps.
///
/// # Errors
///
/// Rejects an incomplete or internally inconsistent resealed package.
pub fn reseal_ontology_program_package(
    package: &mut OntologyProgramPackage,
) -> Result<(), OntologyProgramError> {
    refresh_bootstrap_member(package)?;
    package.manifest = manifest_for_members(
        &package.manifest.packaging_profile_id,
        &package.members,
        &package.runtime_artifacts,
    );
    validate_ontology_program_package(package)
}

#[cfg(test)]
pub(crate) fn replace_program_utf8_cell(
    package: &mut OntologyProgramPackage,
    relation: &str,
    column: &str,
    row: usize,
    replacement: &str,
) -> Result<(), OntologyProgramError> {
    let member = package
        .members
        .get_mut(relation)
        .ok_or_else(|| OntologyProgramError::Contract(format!("missing {relation}")))?;
    if member.batches.len() != 1 {
        return Err(OntologyProgramError::Contract(format!(
            "{relation} is not a canonical single-batch member"
        )));
    }
    let batch = &member.batches[0];
    let column_index = batch
        .schema()
        .index_of(column)
        .map_err(OntologyProgramError::Arrow)?;
    let source = batch.columns()[column_index]
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| {
            OntologyProgramError::Contract(format!("{relation}.{column} is not Utf8"))
        })?;
    if row >= source.len() {
        return Err(OntologyProgramError::Contract(format!(
            "{relation}.{column}[{row}] is outside the member"
        )));
    }
    let values = (0..source.len())
        .map(|index| {
            if index == row {
                Some(replacement)
            } else if source.is_null(index) {
                None
            } else {
                Some(source.value(index))
            }
        })
        .collect::<Vec<_>>();
    let mut columns = batch.columns().to_vec();
    columns[column_index] = std::sync::Arc::new(StringArray::from(values));
    let replacement_batch = RecordBatch::try_new(std::sync::Arc::clone(&member.schema), columns)?;
    replace_member_batch(member, replacement_batch)?;
    reseal_ontology_program_package(package)
}

#[cfg(test)]
pub(crate) fn replace_program_bool_cell(
    package: &mut OntologyProgramPackage,
    relation: &str,
    column: &str,
    row: usize,
    replacement: bool,
) -> Result<(), OntologyProgramError> {
    let member = package
        .members
        .get_mut(relation)
        .ok_or_else(|| OntologyProgramError::Contract(format!("missing {relation}")))?;
    if member.batches.len() != 1 {
        return Err(OntologyProgramError::Contract(format!(
            "{relation} is not a canonical single-batch member"
        )));
    }
    let batch = &member.batches[0];
    let column_index = batch
        .schema()
        .index_of(column)
        .map_err(OntologyProgramError::Arrow)?;
    let source = batch.columns()[column_index]
        .as_any()
        .downcast_ref::<BooleanArray>()
        .ok_or_else(|| {
            OntologyProgramError::Contract(format!("{relation}.{column} is not Boolean"))
        })?;
    if row >= source.len() {
        return Err(OntologyProgramError::Contract(format!(
            "{relation}.{column}[{row}] is outside the member"
        )));
    }
    let values = (0..source.len())
        .map(|index| {
            if index == row {
                Some(replacement)
            } else if source.is_null(index) {
                None
            } else {
                Some(source.value(index))
            }
        })
        .collect::<Vec<_>>();
    let mut columns = batch.columns().to_vec();
    columns[column_index] = std::sync::Arc::new(BooleanArray::from(values));
    let replacement_batch = RecordBatch::try_new(std::sync::Arc::clone(&member.schema), columns)?;
    replace_member_batch(member, replacement_batch)?;
    reseal_ontology_program_package(package)
}

fn refresh_bootstrap_member(
    package: &mut OntologyProgramPackage,
) -> Result<(), OntologyProgramError> {
    let content_rows = package
        .members
        .iter()
        .filter(|(relation, _)| relation.as_str() != "program.bootstrap")
        .map(|(relation, member)| {
            (
                relation.clone(),
                format!(
                    "b3:{}",
                    blake3::hash(format!("{:?}", member.schema).as_bytes()).to_hex()
                ),
                format!("b3:{}", blake3::hash(&member.ipc_bytes).to_hex()),
            )
        })
        .collect::<Vec<_>>();
    let mut content_set = blake3::Hasher::new();
    content_set.update(b"codefabric.ontology-program.content-set.v1\0");
    for (relation, schema_identity, content_identity) in &content_rows {
        for value in [relation, schema_identity, content_identity] {
            content_set.update(&(value.len() as u64).to_be_bytes());
            content_set.update(value.as_bytes());
        }
    }
    let content_set_identity = format!("b3:{}", content_set.finalize().to_hex());
    let bootstrap = package
        .members
        .get_mut("program.bootstrap")
        .ok_or_else(|| OntologyProgramError::Contract("missing program.bootstrap".into()))?;
    let batch = RecordBatch::try_new(
        std::sync::Arc::clone(&bootstrap.schema),
        vec![
            std::sync::Arc::new(StringArray::from_iter_values(
                content_rows.iter().map(|row| row.0.as_str()),
            )),
            std::sync::Arc::new(StringArray::from_iter_values(
                content_rows.iter().map(|row| row.0.as_str()),
            )),
            std::sync::Arc::new(StringArray::from(vec![
                "program_relation";
                content_rows.len()
            ])),
            std::sync::Arc::new(StringArray::from_iter_values(
                content_rows.iter().map(|row| row.1.as_str()),
            )),
            std::sync::Arc::new(StringArray::from_iter_values(
                content_rows.iter().map(|row| row.2.as_str()),
            )),
            std::sync::Arc::new(BooleanArray::from(vec![true; content_rows.len()])),
            std::sync::Arc::new(StringArray::from(vec![
                content_set_identity.as_str();
                content_rows.len()
            ])),
        ],
    )?;
    replace_member_batch(bootstrap, batch)
}

fn replace_member_batch(
    member: &mut OntologyProgramMember,
    batch: RecordBatch,
) -> Result<(), OntologyProgramError> {
    let mut ipc_bytes = Vec::new();
    {
        let mut writer =
            arrow_ipc::writer::StreamWriter::try_new(&mut ipc_bytes, member.schema.as_ref())?;
        writer.write(&batch)?;
        writer.finish()?;
    }
    member.batches = vec![batch];
    member.ipc_bytes = ipc_bytes;
    member.member_identity = framed([member.relation_id.as_bytes(), member.ipc_bytes.as_slice()]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use arrow_array::{Array as _, StringArray};

    use super::{
        CandidateProgramBinding, EpochRuntimeAuthority, OntologyPackagingProfile,
        RESULT_CONTRACT_SET_ARTIFACT, build_ontology_program_package,
        install_ontology_program_package, load_installed_ontology_program_package,
        replace_program_utf8_cell, reseal_ontology_program_package, result_checksum_version,
        validate_ontology_program_package,
    };
    use crate::ontology_executor::OntologyProgramCompiler;

    #[test]
    fn ontology_program_bundle_semantic_parity() {
        let package = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("compiled package");
        for relation in [
            "program.program_contract",
            "program.scan_node",
            "program.filter_node",
            "program.project_node",
            "program.join_node",
            "program.aggregate_node",
            "program.set_node",
            "program.column_expr",
            "program.literal_expr",
            "program.binary_expr",
            "program.call_expr",
            "program.case_expr",
            "program.cast_expr",
            "program.plan_edge",
            "program.expression_edge",
        ] {
            assert!(package.members.contains_key(relation), "missing {relation}");
        }
        let rules = &package.members["program.rule_binding"];
        let compiler = OntologyProgramCompiler::decode(&package).expect("decoded program");
        assert_eq!(rules.batches[0].num_rows(), compiler.rules.values().count());
        assert!(
            compiler
                .rules
                .values()
                .all(|rule| !rule.calculation_id.is_empty()
                    && !rule.policy_id.is_empty()
                    && !rule.input_contract.is_empty())
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
        let calculation_rows =
            package.members["program.calculation_contract"].batches[0].num_rows();
        let compiler = OntologyProgramCompiler::decode(&package).expect("decoded program");
        assert_eq!(calculation_rows, compiler.calculations.len());
        assert!(
            package
                .manifest
                .authored_content_identity
                .starts_with("b3:")
        );
    }

    #[test]
    fn ontology_retained_epoch_package_reconstruction() {
        let root = tempfile::tempdir().expect("retained package root");
        let first = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("first retained package");
        let mut second = first.clone();
        let phrase_batch = &second.members["program.phrase_binding"].batches[0];
        let phrase_ids = phrase_batch
            .column_by_name("phrase_id")
            .and_then(|column| column.as_any().downcast_ref::<StringArray>())
            .expect("phrase IDs");
        let phrase_rows = (0..phrase_ids.len())
            .filter(|&row| phrase_ids.value(row) == "CONDITION_CERTAINTY_EXACT")
            .collect::<Vec<_>>();
        assert!(!phrase_rows.is_empty());
        for row in phrase_rows {
            replace_program_utf8_cell(
                &mut second,
                "program.phrase_binding",
                "canonical_text",
                row,
                "certainty is retained-exact",
            )
            .expect("resealed retained package");
        }
        assert_ne!(
            first.manifest.package_identity,
            second.manifest.package_identity
        );

        install_ontology_program_package(root.path(), &first).expect("install first package");
        install_ontology_program_package(root.path(), &second).expect("install second package");
        let loaded_first =
            load_installed_ontology_program_package(root.path(), &first.manifest.package_identity)
                .expect("reconstruct first package");
        let loaded_second =
            load_installed_ontology_program_package(root.path(), &second.manifest.package_identity)
                .expect("reconstruct second package");
        let first_compiler =
            OntologyProgramCompiler::decode(&loaded_first).expect("first compiler");
        let second_compiler =
            OntologyProgramCompiler::decode(&loaded_second).expect("second compiler");
        assert!(
            first_compiler
                .lower_phrase_text("certainty is exact")
                .is_ok()
        );
        assert!(
            second_compiler
                .lower_phrase_text("certainty is retained-exact")
                .is_ok()
        );
        assert!(
            second_compiler
                .lower_phrase_text("certainty is exact")
                .is_err()
        );

        let second_root = root
            .path()
            .join("ontology-programs")
            .join(second.manifest.package_identity.trim_start_matches("b3:"));
        let manifest_path = second_root.join("manifest.json");
        let result_contract_path = second_root.join(RESULT_CONTRACT_SET_ARTIFACT);
        let result_contract_bytes =
            std::fs::read(&result_contract_path).expect("read retained result-contract artifact");
        let mut corrupted_result_contract = result_contract_bytes.clone();
        corrupted_result_contract.push(b'\n');
        std::fs::write(&result_contract_path, corrupted_result_contract)
            .expect("corrupt retained result-contract artifact");
        assert!(
            load_installed_ontology_program_package(root.path(), &second.manifest.package_identity)
                .is_err(),
            "retained runtime artifact tamper must fail before session construction"
        );
        std::fs::write(&result_contract_path, result_contract_bytes)
            .expect("restore retained result-contract artifact");
        std::fs::write(&manifest_path, b"{}\n").expect("corrupt retained manifest");
        assert!(
            load_installed_ontology_program_package(root.path(), &second.manifest.package_identity)
                .is_err(),
            "corrupt retained package must not fall back to current binary state"
        );
        assert!(
            load_installed_ontology_program_package(
                root.path(),
                "b3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            )
            .is_err(),
            "missing retained package must fail closed"
        );
    }

    #[test]
    fn epoch_runtime_schema_variant_is_decoded_and_result_authority_bound() {
        let base = build_ontology_program_package(&OntologyPackagingProfile::default())
            .expect("base epoch package");
        let mut variant = base.clone();
        let bytes = variant
            .runtime_artifacts
            .get_mut(RESULT_CONTRACT_SET_ARTIFACT)
            .expect("result contracts");
        let mut value: serde_json::Value =
            serde_json::from_slice(bytes).expect("decode retained schemas");
        value["schemas"]["result.find_entities.v2"]["metadata"]
            .as_object_mut()
            .expect("schema metadata")
            .insert(
                "epoch-test".into(),
                serde_json::Value::String("variant".into()),
            );
        *bytes = crate::contracts::jcs::canonicalize_value(&value)
            .expect("canonical retained schema variant");
        reseal_ontology_program_package(&mut variant).expect("reseal variant");

        let base_runtime = EpochRuntimeAuthority::decode(&base).expect("base runtime");
        let variant_runtime = EpochRuntimeAuthority::decode(&variant).expect("variant runtime");
        assert_ne!(base_runtime.identity(), variant_runtime.identity());
        let compiler = OntologyProgramCompiler::decode(&variant).expect("variant compiler");
        assert_eq!(
            compiler
                .result_schema("result.find_entities.v2")
                .expect("retained variant result schema")
                .metadata()
                .get("epoch-test")
                .map(String::as_str),
            Some("variant")
        );
        let pin = |runtime: &EpochRuntimeAuthority| crate::snapshot::ResultAuthorityPin {
            result_authority_identity: String::new(),
            package_identity: base.manifest.package_identity.clone(),
            epoch_runtime_authority_identity: runtime.identity().to_owned(),
            program_identity: base.manifest.logical_program_identity.clone(),
            function_catalog_identity:
                base.manifest.member_identities["program.calculation_contract"].clone(),
            policy_identity: "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            query_form_identity: base.manifest.member_identities["program.phrase_binding"].clone(),
            checksum_version: result_checksum_version(&base).expect("checksum"),
            exact_table_set_identity:
                "b3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
        };
        assert_ne!(
            crate::snapshot::governed_result_authority_identity(&pin(&base_runtime)),
            crate::snapshot::governed_result_authority_identity(&pin(&variant_runtime))
        );
    }
}
