//! CBEF-v1 identities, public encodings, canonical type terms, and workspace paths.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model_generated::identity_recipes as recipes;
pub use crate::model_generated::identity_recipes::SemanticFingerprintDomain;
use unicode_casefold::UnicodeCaseFold as _;
use unicode_normalization::UnicodeNormalization as _;

const MAGIC: &[u8; 4] = b"CFID";
const FORMAT_VERSION: u8 = 1;

/// Stable source/syntax analysis-context value behind the sole symbolic public ID.
pub const SOURCE_CONTEXT_ID: [u8; 16] = [0xff; 16];

/// Owner-approved CBEF-v1 record domains in declaration/code order.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u16)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityDomain {
    Workspace = 1,
    Repository = 2,
    Worktree = 3,
    AnalysisContext = 4,
    ContextSet = 5,
    SourceFile = 6,
    Owner = 7,
    Entity = 8,
    RelationFact = 9,
    PropertyFact = 10,
    Type = 11,
    Publication = 12,
    ServingSnapshot = 13,
    ResultArtifact = 14,
    SourceContext = 15,
    UnknownRemainder = 16,
    RootAuthorization = 17,
}

impl IdentityDomain {
    /// Decode the closed two-byte registry code.
    fn from_code(code: u16) -> Result<Self, IdentityError> {
        match code {
            1 => Ok(Self::Workspace),
            2 => Ok(Self::Repository),
            3 => Ok(Self::Worktree),
            4 => Ok(Self::AnalysisContext),
            5 => Ok(Self::ContextSet),
            6 => Ok(Self::SourceFile),
            7 => Ok(Self::Owner),
            8 => Ok(Self::Entity),
            9 => Ok(Self::RelationFact),
            10 => Ok(Self::PropertyFact),
            11 => Ok(Self::Type),
            12 => Ok(Self::Publication),
            13 => Ok(Self::ServingSnapshot),
            14 => Ok(Self::ResultArtifact),
            15 => Ok(Self::SourceContext),
            16 => Ok(Self::UnknownRemainder),
            17 => Ok(Self::RootAuthorization),
            _ => Err(IdentityError::UnknownDomain(code)),
        }
    }

    const fn public_prefix(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Repository => "repository",
            Self::Worktree => "worktree",
            Self::AnalysisContext => "context",
            Self::ContextSet => "context-set",
            Self::SourceFile => "file",
            Self::Owner => "owner",
            Self::Entity => "entity",
            Self::RelationFact | Self::PropertyFact => "fact",
            Self::Type => "type",
            Self::Publication => "publication",
            Self::ServingSnapshot => "snapshot",
            Self::ResultArtifact => "artifact",
            Self::SourceContext => "source-context",
            Self::UnknownRemainder => "unknown",
            Self::RootAuthorization => "root-authorization",
        }
    }

    const fn requires_kind_slug(self) -> bool {
        matches!(self, Self::Entity | Self::RelationFact | Self::PropertyFact)
    }
}

/// CBEF-v1 core type code.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CbefTypeCode {
    Absent = 0,
    Bytes = 1,
    Utf8 = 2,
    RawPath = 3,
    Unsigned = 4,
    Signed = 5,
    Boolean = 6,
    Id = 7,
    Digest = 8,
    OrderedList = 9,
    Set = 10,
    Map = 11,
    TaggedUnion = 12,
}

impl CbefTypeCode {
    fn from_code(code: u8) -> Result<Self, IdentityError> {
        match code {
            0 => Ok(Self::Absent),
            1 => Ok(Self::Bytes),
            2 => Ok(Self::Utf8),
            3 => Ok(Self::RawPath),
            4 => Ok(Self::Unsigned),
            5 => Ok(Self::Signed),
            6 => Ok(Self::Boolean),
            7 => Ok(Self::Id),
            8 => Ok(Self::Digest),
            9 => Ok(Self::OrderedList),
            10 => Ok(Self::Set),
            11 => Ok(Self::Map),
            12 => Ok(Self::TaggedUnion),
            _ => Err(IdentityError::UnknownTypeCode(code)),
        }
    }
}

/// Field-schema-declared semantic-string normalization.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StringNormalization {
    None,
    Nfc,
    Nfkc,
    AsciiLower,
    PythonIdentifierNfkc,
    RustCanonical,
}

/// Typed CBEF value. Container members retain their own type codes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CbefValue {
    Absent,
    Bytes(Vec<u8>),
    Utf8 {
        value: String,
        normalization: StringNormalization,
    },
    RawPath {
        platform_code: u8,
        bytes: Vec<u8>,
    },
    Unsigned(Vec<u8>),
    Signed(Vec<u8>),
    Boolean(bool),
    Id([u8; 16]),
    Digest([u8; 32]),
    OrderedList(Vec<Self>),
    Set(Vec<Self>),
    Map(Vec<(Self, Self)>),
    TaggedUnion {
        variant: u16,
        value: Box<Self>,
    },
}

impl CbefValue {
    fn type_code(&self) -> CbefTypeCode {
        match self {
            Self::Absent => CbefTypeCode::Absent,
            Self::Bytes(_) => CbefTypeCode::Bytes,
            Self::Utf8 { .. } => CbefTypeCode::Utf8,
            Self::RawPath { .. } => CbefTypeCode::RawPath,
            Self::Unsigned(_) => CbefTypeCode::Unsigned,
            Self::Signed(_) => CbefTypeCode::Signed,
            Self::Boolean(_) => CbefTypeCode::Boolean,
            Self::Id(_) => CbefTypeCode::Id,
            Self::Digest(_) => CbefTypeCode::Digest,
            Self::OrderedList(_) => CbefTypeCode::OrderedList,
            Self::Set(_) => CbefTypeCode::Set,
            Self::Map(_) => CbefTypeCode::Map,
            Self::TaggedUnion { .. } => CbefTypeCode::TaggedUnion,
        }
    }
}

/// One tagged field in a CBEF record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CbefField {
    pub tag: u16,
    pub value: CbefValue,
}

/// Decoded closed CBEF record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CbefRecord {
    pub domain: IdentityDomain,
    pub fields: Vec<CbefField>,
}

/// Full and truncated identities retained together for collision diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedIdentity {
    pub id: [u8; 16],
    pub full_digest: [u8; 32],
    pub preimage: Vec<u8>,
}

/// Stable identity/path validation failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum IdentityError {
    #[error("invalid CBEF magic or version")]
    Header,
    #[error("unknown CBEF domain code {0}")]
    UnknownDomain(u16),
    #[error("unknown CBEF type code {0}")]
    UnknownTypeCode(u8),
    #[error("truncated CBEF payload")]
    Truncated,
    #[error("CBEF fields are duplicate or nonascending")]
    FieldOrder,
    #[error("CBEF container is noncanonical")]
    ContainerOrder,
    #[error("CBEF scalar width or value is invalid")]
    Scalar,
    #[error("CBEF record contains trailing bytes")]
    TrailingBytes,
    #[error("public ID has invalid prefix, slug, case, or width")]
    PublicId,
    #[error("ID_COLLISION for {id}")]
    IdCollision { id: String },
    #[error("workspace path has an invalid component encoding")]
    PathEncoding,
    #[error("workspace path uses unsupported platform code {0}")]
    Platform(u8),
    #[error("workspace comparison-key collision")]
    PathCollision,
    #[error("filesystem case-sensitivity probe failed: {0}")]
    CaseProbe(String),
    #[error("registration nonce generation failed: {0}")]
    Random(String),
}

fn normalized(value: &str, rule: StringNormalization) -> Result<String, IdentityError> {
    Ok(match rule {
        StringNormalization::None | StringNormalization::RustCanonical => value.to_owned(),
        StringNormalization::Nfc => value.nfc().collect(),
        StringNormalization::Nfkc | StringNormalization::PythonIdentifierNfkc => {
            value.nfkc().collect()
        }
        StringNormalization::AsciiLower => {
            if !value.is_ascii() {
                return Err(IdentityError::Scalar);
            }
            value.to_ascii_lowercase()
        }
    })
}

fn put_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend(value.to_be_bytes());
}

fn put_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), IdentityError> {
    bytes.extend(
        u32::try_from(value)
            .map_err(|_| IdentityError::Scalar)?
            .to_be_bytes(),
    );
    Ok(())
}

fn encode_typed(value: &CbefValue) -> Result<Vec<u8>, IdentityError> {
    let payload = encode_payload(value)?;
    let mut encoded = vec![value.type_code() as u8];
    put_u32(&mut encoded, payload.len())?;
    encoded.extend(payload);
    Ok(encoded)
}

fn encode_payload(value: &CbefValue) -> Result<Vec<u8>, IdentityError> {
    match value {
        CbefValue::Absent => Ok(Vec::new()),
        CbefValue::Bytes(bytes) => Ok(bytes.clone()),
        CbefValue::Utf8 {
            value,
            normalization,
        } => Ok(normalized(value, *normalization)?.into_bytes()),
        CbefValue::RawPath {
            platform_code,
            bytes,
        } => {
            if !matches!(platform_code, 1..=3) {
                return Err(IdentityError::Platform(*platform_code));
            }
            let mut payload = vec![*platform_code];
            payload.extend(bytes);
            Ok(payload)
        }
        CbefValue::Unsigned(bytes) | CbefValue::Signed(bytes) => {
            if !matches!(bytes.len(), 1 | 2 | 4 | 8 | 16) {
                return Err(IdentityError::Scalar);
            }
            Ok(bytes.clone())
        }
        CbefValue::Boolean(value) => Ok(vec![u8::from(*value)]),
        CbefValue::Id(value) => Ok(value.to_vec()),
        CbefValue::Digest(value) => Ok(value.to_vec()),
        CbefValue::OrderedList(values) => encode_sequence(values, false),
        CbefValue::Set(values) => encode_sequence(values, true),
        CbefValue::Map(entries) => {
            let mut entries = entries
                .iter()
                .map(|(key, value)| Ok((encode_typed(key)?, encode_typed(value)?)))
                .collect::<Result<Vec<_>, IdentityError>>()?;
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(IdentityError::ContainerOrder);
            }
            let mut payload = Vec::new();
            put_u32(&mut payload, entries.len())?;
            for (key, value) in entries {
                put_u32(&mut payload, key.len())?;
                payload.extend(key);
                put_u32(&mut payload, value.len())?;
                payload.extend(value);
            }
            Ok(payload)
        }
        CbefValue::TaggedUnion { variant, value } => {
            let encoded = encode_typed(value)?;
            let mut payload = Vec::new();
            put_u16(&mut payload, *variant);
            put_u32(&mut payload, encoded.len())?;
            payload.extend(encoded);
            Ok(payload)
        }
    }
}

fn encode_sequence(values: &[CbefValue], sorted: bool) -> Result<Vec<u8>, IdentityError> {
    let mut values = values
        .iter()
        .map(encode_typed)
        .collect::<Result<Vec<_>, _>>()?;
    if sorted {
        values.sort();
        values.dedup();
    }
    let mut payload = Vec::new();
    put_u32(&mut payload, values.len())?;
    for value in values {
        put_u32(&mut payload, value.len())?;
        payload.extend(value);
    }
    Ok(payload)
}

/// Encode a strictly ascending CBEF-v1 record.
///
/// # Errors
///
/// Returns an error for nonascending tags, invalid values, or framing-width overflow.
pub fn encode_record(record: &CbefRecord) -> Result<Vec<u8>, IdentityError> {
    if record.fields.first().is_some_and(|field| field.tag == 0)
        || record
            .fields
            .windows(2)
            .any(|pair| pair[0].tag >= pair[1].tag)
    {
        return Err(IdentityError::FieldOrder);
    }
    let mut encoded = MAGIC.to_vec();
    encoded.push(FORMAT_VERSION);
    put_u16(&mut encoded, record.domain as u16);
    put_u16(
        &mut encoded,
        u16::try_from(record.fields.len()).map_err(|_| IdentityError::Scalar)?,
    );
    for field in &record.fields {
        put_u16(&mut encoded, field.tag);
        encoded.push(field.value.type_code() as u8);
        let payload = encode_payload(&field.value)?;
        put_u32(&mut encoded, payload.len())?;
        encoded.extend(payload);
    }
    Ok(encoded)
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, count: usize) -> Result<&'a [u8], IdentityError> {
    let end = cursor.checked_add(count).ok_or(IdentityError::Truncated)?;
    let value = bytes.get(*cursor..end).ok_or(IdentityError::Truncated)?;
    *cursor = end;
    Ok(value)
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, IdentityError> {
    Ok(u16::from_be_bytes(
        take(bytes, cursor, 2)?.try_into().expect("exact slice"),
    ))
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<usize, IdentityError> {
    Ok(usize::try_from(u32::from_be_bytes(
        take(bytes, cursor, 4)?.try_into().expect("exact slice"),
    ))
    .expect("u32 fits usize"))
}

fn decode_typed(bytes: &[u8]) -> Result<CbefValue, IdentityError> {
    let mut cursor = 0;
    let type_code = CbefTypeCode::from_code(
        take(bytes, &mut cursor, 1)?
            .first()
            .copied()
            .ok_or(IdentityError::Truncated)?,
    )?;
    let length = read_u32(bytes, &mut cursor)?;
    let payload = take(bytes, &mut cursor, length)?;
    if cursor != bytes.len() {
        return Err(IdentityError::TrailingBytes);
    }
    decode_payload(type_code, payload)
}

fn decode_payload(type_code: CbefTypeCode, payload: &[u8]) -> Result<CbefValue, IdentityError> {
    match type_code {
        CbefTypeCode::Absent if payload.is_empty() => Ok(CbefValue::Absent),
        CbefTypeCode::Bytes => Ok(CbefValue::Bytes(payload.to_vec())),
        CbefTypeCode::Utf8 => Ok(CbefValue::Utf8 {
            value: std::str::from_utf8(payload)
                .map_err(|_| IdentityError::Scalar)?
                .to_owned(),
            normalization: StringNormalization::None,
        }),
        CbefTypeCode::RawPath => {
            let (&platform_code, bytes) = payload.split_first().ok_or(IdentityError::Scalar)?;
            if !matches!(platform_code, 1..=3) {
                return Err(IdentityError::Platform(platform_code));
            }
            Ok(CbefValue::RawPath {
                platform_code,
                bytes: bytes.to_vec(),
            })
        }
        CbefTypeCode::Unsigned | CbefTypeCode::Signed
            if matches!(payload.len(), 1 | 2 | 4 | 8 | 16) =>
        {
            Ok(if type_code == CbefTypeCode::Unsigned {
                CbefValue::Unsigned(payload.to_vec())
            } else {
                CbefValue::Signed(payload.to_vec())
            })
        }
        CbefTypeCode::Boolean if matches!(payload, [0 | 1]) => {
            Ok(CbefValue::Boolean(payload[0] == 1))
        }
        CbefTypeCode::Id if payload.len() == 16 => {
            Ok(CbefValue::Id(payload.try_into().expect("length checked")))
        }
        CbefTypeCode::Digest if payload.len() == 32 => Ok(CbefValue::Digest(
            payload.try_into().expect("length checked"),
        )),
        CbefTypeCode::OrderedList => Ok(CbefValue::OrderedList(decode_sequence(payload, false)?)),
        CbefTypeCode::Set => Ok(CbefValue::Set(decode_sequence(payload, true)?)),
        CbefTypeCode::Map => Ok(CbefValue::Map(decode_map(payload)?)),
        CbefTypeCode::TaggedUnion => {
            let mut cursor = 0;
            let variant = read_u16(payload, &mut cursor)?;
            let length = read_u32(payload, &mut cursor)?;
            let value = decode_typed(take(payload, &mut cursor, length)?)?;
            if cursor != payload.len() {
                return Err(IdentityError::TrailingBytes);
            }
            Ok(CbefValue::TaggedUnion {
                variant,
                value: Box::new(value),
            })
        }
        CbefTypeCode::Absent
        | CbefTypeCode::Unsigned
        | CbefTypeCode::Signed
        | CbefTypeCode::Boolean
        | CbefTypeCode::Id
        | CbefTypeCode::Digest => Err(IdentityError::Scalar),
    }
}

fn decode_sequence(payload: &[u8], sorted: bool) -> Result<Vec<CbefValue>, IdentityError> {
    let mut cursor = 0;
    let count = read_u32(payload, &mut cursor)?;
    let mut encoded = Vec::with_capacity(count);
    for _ in 0..count {
        let length = read_u32(payload, &mut cursor)?;
        let item = take(payload, &mut cursor, length)?;
        let _ = decode_typed(item)?;
        encoded.push(item.to_vec());
    }
    if cursor != payload.len() {
        return Err(IdentityError::TrailingBytes);
    }
    if sorted && encoded.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(IdentityError::ContainerOrder);
    }
    encoded.iter().map(|item| decode_typed(item)).collect()
}

fn decode_map(payload: &[u8]) -> Result<Vec<(CbefValue, CbefValue)>, IdentityError> {
    let mut cursor = 0;
    let count = read_u32(payload, &mut cursor)?;
    let mut entries = Vec::with_capacity(count);
    let mut previous = None;
    for _ in 0..count {
        let key_length = read_u32(payload, &mut cursor)?;
        let key = take(payload, &mut cursor, key_length)?;
        if previous.is_some_and(|value: &[u8]| value >= key) {
            return Err(IdentityError::ContainerOrder);
        }
        let value_length = read_u32(payload, &mut cursor)?;
        let value = take(payload, &mut cursor, value_length)?;
        entries.push((decode_typed(key)?, decode_typed(value)?));
        previous = Some(key);
    }
    if cursor != payload.len() {
        return Err(IdentityError::TrailingBytes);
    }
    Ok(entries)
}

/// Decode one complete CBEF record and reject all noncanonical framing.
///
/// # Errors
///
/// Returns an error for unknown codes, invalid scalar/container framing, truncation,
/// nonascending tags, or trailing bytes.
pub fn decode_record(bytes: &[u8]) -> Result<CbefRecord, IdentityError> {
    let mut cursor = 0;
    if take(bytes, &mut cursor, 4)? != MAGIC || take(bytes, &mut cursor, 1)? != [FORMAT_VERSION] {
        return Err(IdentityError::Header);
    }
    let domain = IdentityDomain::from_code(read_u16(bytes, &mut cursor)?)?;
    let count = read_u16(bytes, &mut cursor)?;
    let mut fields = Vec::with_capacity(usize::from(count));
    let mut previous = 0;
    for _ in 0..count {
        let tag = read_u16(bytes, &mut cursor)?;
        if tag <= previous {
            return Err(IdentityError::FieldOrder);
        }
        let type_code = CbefTypeCode::from_code(
            take(bytes, &mut cursor, 1)?
                .first()
                .copied()
                .ok_or(IdentityError::Truncated)?,
        )?;
        let length = read_u32(bytes, &mut cursor)?;
        let payload = take(bytes, &mut cursor, length)?;
        fields.push(CbefField {
            tag,
            value: decode_payload(type_code, payload)?,
        });
        previous = tag;
    }
    if cursor != bytes.len() {
        return Err(IdentityError::TrailingBytes);
    }
    Ok(CbefRecord { domain, fields })
}

/// Derive BLAKE3-256 and its canonical 16-byte public identity.
///
/// # Errors
///
/// Returns an error when the source record is not canonically encodable.
pub fn derive_identity(record: &CbefRecord) -> Result<DerivedIdentity, IdentityError> {
    let preimage = encode_record(record)?;
    let full_digest = *blake3::hash(&preimage).as_bytes();
    let mut id = [0; 16];
    id.copy_from_slice(&full_digest[..16]);
    Ok(DerivedIdentity {
        id,
        full_digest,
        preimage,
    })
}

/// Registry-selected semantic fingerprint construction. The domain bytes are
/// generated from the governed fingerprint-domain registry; callers may add
/// only the record's declared fields in their declared order.
pub struct SemanticFingerprintBuilder(blake3::Hasher);

impl SemanticFingerprintBuilder {
    /// Add exact bytes to the registered semantic preimage.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Finish as the full persisted semantic fingerprint.
    #[must_use]
    pub fn finalize(self) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }

    /// Finish as the canonical first-16-byte semantic identifier.
    ///
    /// # Panics
    ///
    /// The fixed 32-byte BLAKE3 digest always contains the required 16-byte prefix.
    #[must_use]
    pub fn finalize_id16(self) -> [u8; 16] {
        let digest = self.finalize();
        digest[..16].try_into().expect("exact identity prefix")
    }
}

/// Begin a semantic fingerprint for one generated registry domain.
#[must_use]
pub fn semantic_fingerprint(domain: SemanticFingerprintDomain) -> SemanticFingerprintBuilder {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.bytes());
    SemanticFingerprintBuilder(hasher)
}

/// Derive the shared source-provider capability-scope fingerprint.
#[must_use]
pub fn capability_scope_fingerprint(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    owner_id: [u8; 16],
    source_generation: i64,
    capability_code: &str,
) -> [u8; 32] {
    let mut fingerprint = semantic_fingerprint(SemanticFingerprintDomain::CapabilityScope);
    fingerprint.update(&workspace_id);
    fingerprint.update(&analysis_context_id);
    fingerprint.update(&owner_id);
    fingerprint.update(&source_generation.to_be_bytes());
    fingerprint.update(capability_code.as_bytes());
    fingerprint.finalize()
}

/// Derive the Rust compiler capability coverage fingerprint.
#[must_use]
pub fn rustc_capability_scope_fingerprint(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    source_generation: i64,
    owner_id: [u8; 16],
    chunk_digest: &str,
) -> [u8; 32] {
    let mut fingerprint = semantic_fingerprint(SemanticFingerprintDomain::RustcCapabilityScope);
    fingerprint.update(&workspace_id);
    fingerprint.update(&analysis_context_id);
    fingerprint.update(&source_generation.to_be_bytes());
    fingerprint.update(&owner_id);
    fingerprint.update(chunk_digest.as_bytes());
    fingerprint.finalize()
}

/// Derive the canonical fact-evidence identity used by every ingest lane.
#[must_use]
pub fn fact_evidence_id(
    provider_run_id: [u8; 16],
    observation_id: [u8; 16],
    fact_id: [u8; 16],
) -> [u8; 16] {
    let mut fingerprint = semantic_fingerprint(SemanticFingerprintDomain::FactEvidence);
    fingerprint.update(&provider_run_id);
    fingerprint.update(&observation_id);
    fingerprint.update(&fact_id);
    fingerprint.finalize_id16()
}

/// Derive the canonical source observation identity.
#[must_use]
pub fn source_observation_id(provider_run_id: [u8; 16], family: u8, ordinal: u64) -> [u8; 16] {
    let mut fingerprint = semantic_fingerprint(SemanticFingerprintDomain::SourceObservation);
    fingerprint.update(&provider_run_id);
    fingerprint.update(&[family]);
    fingerprint.update(&ordinal.to_be_bytes());
    fingerprint.finalize_id16()
}

/// Preserve a legacy unframed semantic ID preimage under its explicit registry
/// record. New semantic domains must use a non-empty domain separator.
#[must_use]
pub fn unframed_semantic_id(preimage: &[u8]) -> [u8; 16] {
    let mut fingerprint = semantic_fingerprint(SemanticFingerprintDomain::UnframedId16);
    fingerprint.update(preimage);
    fingerprint.finalize_id16()
}

/// Generate one operating-system-random 128-bit registration nonce.
///
/// # Errors
///
/// Returns an error when the operating-system random source cannot fill the nonce.
pub fn random_registration_nonce() -> Result<[u8; 16], IdentityError> {
    let mut nonce = [0_u8; 16];
    let mut source = std::fs::File::open("/dev/urandom")
        .map_err(|error| IdentityError::Random(error.to_string()))?;
    std::io::Read::read_exact(&mut source, &mut nonce)
        .map_err(|error| IdentityError::Random(error.to_string()))?;
    Ok(nonce)
}

/// Derive the AC-G-09 workspace registration identity.
///
/// # Errors
///
/// Returns an error when the workspace kind is not valid canonical `ASCII` text.
pub fn workspace_registration_identity(
    registration_nonce: [u8; 16],
    workspace_kind: &str,
) -> Result<DerivedIdentity, IdentityError> {
    derive_identity(&CbefRecord {
        domain: IdentityDomain::Workspace,
        fields: vec![
            CbefField {
                tag: 1,
                value: CbefValue::Bytes(registration_nonce.to_vec()),
            },
            CbefField {
                tag: 2,
                value: CbefValue::Utf8 {
                    value: workspace_kind.to_owned(),
                    normalization: StringNormalization::AsciiLower,
                },
            },
        ],
    })
}

/// Derive the AC-G-09 repository registration identity.
///
/// # Errors
///
/// Returns an error only if the closed CBEF recipe cannot be encoded.
pub fn repository_registration_identity(
    registration_nonce: [u8; 16],
) -> Result<DerivedIdentity, IdentityError> {
    derive_identity(&CbefRecord {
        domain: IdentityDomain::Repository,
        fields: vec![CbefField {
            tag: 1,
            value: CbefValue::Bytes(registration_nonce.to_vec()),
        }],
    })
}

/// Derive the AC-G-09 worktree registration identity.
///
/// # Errors
///
/// Returns an error when the worktree kind is not valid canonical `ASCII` text.
pub fn worktree_registration_identity(
    repository_id: [u8; 16],
    registration_nonce: [u8; 16],
    worktree_kind: &str,
) -> Result<DerivedIdentity, IdentityError> {
    derive_identity(&CbefRecord {
        domain: IdentityDomain::Worktree,
        fields: vec![
            CbefField {
                tag: 1,
                value: CbefValue::Id(repository_id),
            },
            CbefField {
                tag: 2,
                value: CbefValue::Bytes(registration_nonce.to_vec()),
            },
            CbefField {
                tag: 3,
                value: CbefValue::Utf8 {
                    value: worktree_kind.to_owned(),
                    normalization: StringNormalization::AsciiLower,
                },
            },
        ],
    })
}

/// Derive the context-set identity for one workspace and its exact context membership.
///
/// # Errors
///
/// Returns an error only if the closed CBEF recipe cannot be encoded.
pub fn context_set_identity(
    workspace_id: [u8; 16],
    context_ids: &[[u8; 16]],
) -> Result<DerivedIdentity, IdentityError> {
    derive_identity(&CbefRecord {
        domain: IdentityDomain::ContextSet,
        fields: vec![
            CbefField {
                tag: 1,
                value: CbefValue::Id(workspace_id),
            },
            CbefField {
                tag: 2,
                value: CbefValue::Set(context_ids.iter().copied().map(CbefValue::Id).collect()),
            },
        ],
    })
}

/// Derive the AC-G-12 present-state source-file identity from its comparison key.
///
/// # Errors
///
/// Returns an error only if the closed CBEF source-file recipe cannot be encoded.
pub fn source_file_identity(path: &WorkspacePath) -> Result<DerivedIdentity, IdentityError> {
    derive_identity(&CbefRecord {
        domain: IdentityDomain::SourceFile,
        fields: vec![
            CbefField {
                tag: 1,
                value: CbefValue::Id(path.workspace_id),
            },
            CbefField {
                tag: 2,
                value: CbefValue::Id(SOURCE_CONTEXT_ID),
            },
            CbefField {
                tag: 3,
                value: CbefValue::Bytes(path.comparison_key_bytes.clone()),
            },
        ],
    })
}

/// Derive one governed semantic owner identity from its closed owner recipe.
///
/// # Errors
///
/// Rejects an invalid owner-kind normalization or a recipe-incompatible value.
pub fn semantic_owner_identity(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    owner_kind: &str,
    semantic_key: Vec<u8>,
) -> Result<DerivedIdentity, IdentityError> {
    let record = recipes::owner(recipes::OwnerFields {
        workspace_id: recipes::RecipeValue::Id(workspace_id),
        analysis_context_id: recipes::RecipeValue::Id(analysis_context_id),
        owner_kind: recipes::RecipeValue::Utf8(owner_kind.to_owned()),
        semantic_key: recipes::RecipeValue::Bytes(semantic_key),
    })
    .map_err(|_| IdentityError::Scalar)?;
    derive_recipe_identity(record)
}

/// Derive one governed semantic entity identity from its closed entity recipe.
///
/// # Errors
///
/// Rejects a zero kind code or a recipe-incompatible semantic key.
pub fn semantic_entity_identity(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    kind_code: u16,
    owner_id: [u8; 16],
    semantic_key: Vec<u8>,
) -> Result<DerivedIdentity, IdentityError> {
    if kind_code == 0 {
        return Err(IdentityError::Scalar);
    }
    let record = recipes::entity(recipes::EntityFields {
        workspace_id: recipes::RecipeValue::Id(workspace_id),
        analysis_context_id: recipes::RecipeValue::Id(analysis_context_id),
        kind_code: recipes::RecipeValue::Unsigned(kind_code.to_be_bytes().to_vec()),
        owner_id: recipes::RecipeValue::Id(owner_id),
        semantic_key: recipes::RecipeValue::Bytes(semantic_key),
    })
    .map_err(|_| IdentityError::Scalar)?;
    derive_recipe_identity(record)
}

/// Derive one governed UTF-8 property proposition.
///
/// # Errors
///
/// Rejects a zero property code or a recipe-incompatible canonical value.
pub fn text_property_fact_identity(
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    property_kind_code: u16,
    subject_entity_id: [u8; 16],
    value: &str,
) -> Result<DerivedIdentity, IdentityError> {
    if property_kind_code == 0 {
        return Err(IdentityError::Scalar);
    }
    let normalized = value.nfc().collect::<String>();
    let record = recipes::property_fact(recipes::PropertyFactFields {
        workspace_id: recipes::RecipeValue::Id(workspace_id),
        analysis_context_id: recipes::RecipeValue::Id(analysis_context_id),
        property_kind_code: recipes::RecipeValue::Unsigned(
            property_kind_code.to_be_bytes().to_vec(),
        ),
        subject_entity_id: recipes::RecipeValue::Id(subject_entity_id),
        canonical_value: recipes::RecipeValue::TaggedUnion(
            50,
            Box::new(recipes::RecipeValue::Utf8(normalized)),
        ),
    })
    .map_err(|_| IdentityError::Scalar)?;
    derive_recipe_identity(record)
}

/// Provider-independent source occurrence fields used by AC-G-13 identities.
///
/// Display text and provider-local node handles are deliberately absent. Structural
/// occurrences use their canonical kind/parent/role/ordinal anchor; flat occurrences
/// use their family-local kind and ordinal. The normalized semantic kind keeps
/// incompatible provider observations distinct without admitting provider-local IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceOccurrenceIdentityInput {
    pub workspace_id: [u8; 16],
    pub file_id: [u8; 16],
    pub source_digest: [u8; 32],
    pub start_byte: u64,
    pub end_byte: u64,
    pub owner_id: [u8; 16],
    pub entity_kind_code: u16,
    pub occurrence_family_code: u16,
    pub normalized_kind_code: u32,
    pub parent_id: Option<[u8; 16]>,
    pub role_code: Option<u16>,
    pub ordinal: u32,
}

/// Derive one canonical source occurrence without provider or display identity.
///
/// # Errors
///
/// Returns an error only if the closed CBEF occurrence recipe cannot be encoded.
pub fn source_occurrence_identity(
    input: SourceOccurrenceIdentityInput,
) -> Result<DerivedIdentity, IdentityError> {
    #[derive(Serialize)]
    struct SemanticKey {
        schema_version: u8,
        file_id: [u8; 16],
        source_digest: [u8; 32],
        start_byte: u64,
        end_byte: u64,
        occurrence_family_code: u16,
        normalized_kind_code: u32,
        parent_id: Option<[u8; 16]>,
        role_code: Option<u16>,
        ordinal: u32,
    }
    if input.start_byte > input.end_byte {
        return Err(IdentityError::Scalar);
    }
    let semantic_key_value = serde_json::to_value(SemanticKey {
        schema_version: 1,
        file_id: input.file_id,
        source_digest: input.source_digest,
        start_byte: input.start_byte,
        end_byte: input.end_byte,
        occurrence_family_code: input.occurrence_family_code,
        normalized_kind_code: input.normalized_kind_code,
        parent_id: input.parent_id,
        role_code: input.role_code,
        ordinal: input.ordinal,
    })
    .map_err(|_| IdentityError::Scalar)?;
    let semantic_key =
        serde_json_canonicalizer::to_vec(&semantic_key_value).map_err(|_| IdentityError::Scalar)?;
    let record = recipes::entity(recipes::EntityFields {
        workspace_id: recipes::RecipeValue::Id(input.workspace_id),
        analysis_context_id: recipes::RecipeValue::Id(SOURCE_CONTEXT_ID),
        kind_code: recipes::RecipeValue::Unsigned(input.entity_kind_code.to_be_bytes().to_vec()),
        owner_id: recipes::RecipeValue::Id(input.owner_id),
        semantic_key: recipes::RecipeValue::Bytes(semantic_key),
    })
    .map_err(|_| IdentityError::Scalar)?;
    derive_recipe_identity(record)
}

/// Provider-independent source relation fields used by AC-G-13 identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceRelationIdentityInput {
    pub workspace_id: [u8; 16],
    pub owner_id: [u8; 16],
    pub relation_kind_code: i32,
    pub source_id: [u8; 16],
    pub target_id: [u8; 16],
    pub ordinal: Option<u32>,
    pub role_code: Option<u16>,
}

/// Derive one canonical source-context relationship.
///
/// # Errors
///
/// Returns an error when a negative relation code is supplied or the closed CBEF
/// relation recipe cannot be encoded.
pub fn source_relation_identity(
    input: SourceRelationIdentityInput,
) -> Result<DerivedIdentity, IdentityError> {
    #[derive(Serialize)]
    struct RelationMetadata {
        schema_version: u8,
        owner_id: [u8; 16],
        ordinal: Option<u32>,
        role_code: Option<u16>,
    }
    let relation_kind_code =
        u16::try_from(input.relation_kind_code).map_err(|_| IdentityError::Scalar)?;
    let role_value = serde_json::to_value(RelationMetadata {
        schema_version: 1,
        owner_id: input.owner_id,
        ordinal: input.ordinal,
        role_code: input.role_code,
    })
    .map_err(|_| IdentityError::Scalar)?;
    let role =
        serde_json_canonicalizer::to_string(&role_value).map_err(|_| IdentityError::Scalar)?;
    let record = recipes::relation_fact(recipes::RelationFactFields {
        workspace_id: recipes::RecipeValue::Id(input.workspace_id),
        analysis_context_id: recipes::RecipeValue::Id(SOURCE_CONTEXT_ID),
        relation_kind_code: recipes::RecipeValue::Unsigned(
            relation_kind_code.to_be_bytes().to_vec(),
        ),
        subject_entity_id: recipes::RecipeValue::Id(input.source_id),
        object_entity_id: recipes::RecipeValue::Id(input.target_id),
        role: recipes::RecipeValue::TaggedUnion(1, Box::new(recipes::RecipeValue::Utf8(role))),
    })
    .map_err(|_| IdentityError::Scalar)?;
    derive_recipe_identity(record)
}

fn derive_recipe_identity(record: recipes::RecipeRecord) -> Result<DerivedIdentity, IdentityError> {
    let domain = IdentityDomain::from_code(record.domain_code)?;
    let fields = record
        .fields
        .into_iter()
        .map(|field| {
            Ok(CbefField {
                tag: field.tag,
                value: recipe_value(field.value, field.normalization)?,
            })
        })
        .collect::<Result<Vec<_>, IdentityError>>()?;
    derive_identity(&CbefRecord { domain, fields })
}

fn recipe_value(
    value: recipes::RecipeValue,
    normalization: recipes::RecipeNormalization,
) -> Result<CbefValue, IdentityError> {
    let string_normalization = match normalization {
        recipes::RecipeNormalization::None => StringNormalization::None,
        recipes::RecipeNormalization::Nfc => StringNormalization::Nfc,
        recipes::RecipeNormalization::Nfkc => StringNormalization::Nfkc,
        recipes::RecipeNormalization::AsciiLower => StringNormalization::AsciiLower,
        recipes::RecipeNormalization::PythonIdentifierNfkc => {
            StringNormalization::PythonIdentifierNfkc
        }
        recipes::RecipeNormalization::RustCanonical => StringNormalization::RustCanonical,
    };
    Ok(match value {
        recipes::RecipeValue::Absent => CbefValue::Absent,
        recipes::RecipeValue::Bytes(value) => CbefValue::Bytes(value),
        recipes::RecipeValue::Utf8(value) => CbefValue::Utf8 {
            value,
            normalization: string_normalization,
        },
        recipes::RecipeValue::RawPath(platform_code, bytes) => CbefValue::RawPath {
            platform_code,
            bytes,
        },
        recipes::RecipeValue::Unsigned(value) => CbefValue::Unsigned(value),
        recipes::RecipeValue::Signed(value) => CbefValue::Signed(value),
        recipes::RecipeValue::Boolean(value) => CbefValue::Boolean(value),
        recipes::RecipeValue::Id(value) => CbefValue::Id(value),
        recipes::RecipeValue::Digest(value) => CbefValue::Digest(value),
        recipes::RecipeValue::OrderedList(values) => CbefValue::OrderedList(
            values
                .into_iter()
                .map(|value| recipe_value(value, recipes::RecipeNormalization::None))
                .collect::<Result<_, _>>()?,
        ),
        recipes::RecipeValue::Set(values) => CbefValue::Set(
            values
                .into_iter()
                .map(|value| recipe_value(value, recipes::RecipeNormalization::None))
                .collect::<Result<_, _>>()?,
        ),
        recipes::RecipeValue::Map(values) => CbefValue::Map(
            values
                .into_iter()
                .map(|(key, value)| {
                    Ok((
                        recipe_value(key, recipes::RecipeNormalization::None)?,
                        recipe_value(value, recipes::RecipeNormalization::None)?,
                    ))
                })
                .collect::<Result<_, IdentityError>>()?,
        ),
        recipes::RecipeValue::TaggedUnion(variant, value) => CbefValue::TaggedUnion {
            variant,
            value: Box::new(recipe_value(*value, recipes::RecipeNormalization::None)?),
        },
    })
}

/// AC-G-11 root-authorization fields whose CBEF digest is persisted as the fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootAuthorizationInput {
    pub workspace_id: [u8; 16],
    pub root_path_bytes: Vec<u8>,
    pub root_directory_file_identity: Vec<u8>,
    pub platform_code: u8,
    pub case_sensitivity_mode: String,
    pub authorization_revision: u64,
    pub allowed_source_disclosure_rules: Vec<String>,
}

/// Derive the full AC-G-11 root-authorization fingerprint from one closed CBEF record.
///
/// # Errors
///
/// Returns an error when a path, platform, case mode, or disclosure rule is invalid.
pub fn root_authorization_fingerprint(
    input: &RootAuthorizationInput,
) -> Result<[u8; 32], IdentityError> {
    let identity = derive_identity(&CbefRecord {
        domain: IdentityDomain::RootAuthorization,
        fields: vec![
            CbefField {
                tag: 1,
                value: CbefValue::Id(input.workspace_id),
            },
            CbefField {
                tag: 2,
                value: CbefValue::RawPath {
                    platform_code: input.platform_code,
                    bytes: input.root_path_bytes.clone(),
                },
            },
            CbefField {
                tag: 3,
                value: CbefValue::Bytes(input.root_directory_file_identity.clone()),
            },
            CbefField {
                tag: 4,
                value: CbefValue::Unsigned(vec![input.platform_code]),
            },
            CbefField {
                tag: 5,
                value: CbefValue::Utf8 {
                    value: input.case_sensitivity_mode.clone(),
                    normalization: StringNormalization::AsciiLower,
                },
            },
            CbefField {
                tag: 6,
                value: CbefValue::Unsigned(input.authorization_revision.to_be_bytes().to_vec()),
            },
            CbefField {
                tag: 7,
                value: CbefValue::Set(
                    input
                        .allowed_source_disclosure_rules
                        .iter()
                        .map(|value| CbefValue::Utf8 {
                            value: value.clone(),
                            normalization: StringNormalization::AsciiLower,
                        })
                        .collect(),
                ),
            },
        ],
    })?;
    Ok(identity.full_digest)
}

/// Collision-diagnostic registry retaining full digests and exact preimages.
#[derive(Clone, Debug, Default)]
pub struct IdentityRegistry {
    records: BTreeMap<[u8; 16], ([u8; 32], Vec<u8>)>,
}

impl IdentityRegistry {
    /// Insert one derived identity or return the blocking `ID_COLLISION` class.
    ///
    /// # Errors
    ///
    /// Returns `ID_COLLISION` when the same 128-bit ID has unequal diagnostic evidence.
    pub fn register(&mut self, identity: &DerivedIdentity) -> Result<(), IdentityError> {
        if let Some((digest, preimage)) = self.records.get(&identity.id) {
            if digest != &identity.full_digest || preimage != &identity.preimage {
                return Err(IdentityError::IdCollision {
                    id: lower_hex(&identity.id),
                });
            }
            return Ok(());
        }
        self.records.insert(
            identity.id,
            (identity.full_digest, identity.preimage.clone()),
        );
        Ok(())
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("writing to a String is infallible");
    }
    encoded
}

fn decode_lower_hex_16(value: &str) -> Result<[u8; 16], IdentityError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(IdentityError::PublicId);
    }
    let mut decoded = [0; 16];
    for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        decoded[index] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
            .map_err(|_| IdentityError::PublicId)?;
    }
    Ok(decoded)
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Encode one strict lowercase public identity.
///
/// # Errors
///
/// Returns an error when the domain's kind-slug shape is not satisfied.
pub fn encode_public_id(
    domain: IdentityDomain,
    kind_slug: Option<&str>,
    id: [u8; 16],
) -> Result<String, IdentityError> {
    if domain.requires_kind_slug() != kind_slug.is_some()
        || kind_slug.is_some_and(|slug| !valid_slug(slug))
    {
        return Err(IdentityError::PublicId);
    }
    Ok(match kind_slug {
        Some(slug) => format!("{}:{slug}:{}", domain.public_prefix(), lower_hex(&id)),
        None => format!("{}:{}", domain.public_prefix(), lower_hex(&id)),
    })
}

/// Decode a public identity only when its prefix/domain and optional kind slug match.
///
/// # Errors
///
/// Returns an error for an unexpected prefix, slug, width, case, or symbolic value.
pub fn decode_public_id(
    expected_domain: IdentityDomain,
    expected_kind_slug: Option<&str>,
    value: &str,
) -> Result<[u8; 16], IdentityError> {
    if value == "context:source" {
        return (expected_domain == IdentityDomain::AnalysisContext
            && expected_kind_slug.is_none())
        .then_some(SOURCE_CONTEXT_ID)
        .ok_or(IdentityError::PublicId);
    }
    let expected_prefix = expected_domain.public_prefix();
    let parts = value.split(':').collect::<Vec<_>>();
    let payload = if expected_domain.requires_kind_slug() {
        let [prefix, slug, payload] = parts.as_slice() else {
            return Err(IdentityError::PublicId);
        };
        if *prefix != expected_prefix || !valid_slug(slug) || Some(*slug) != expected_kind_slug {
            return Err(IdentityError::PublicId);
        }
        *payload
    } else {
        let [prefix, payload] = parts.as_slice() else {
            return Err(IdentityError::PublicId);
        };
        if *prefix != expected_prefix || expected_kind_slug.is_some() {
            return Err(IdentityError::PublicId);
        }
        *payload
    };
    decode_lower_hex_16(payload)
}

/// Supported raw-path platform code.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum PlatformCode {
    Unix = 1,
    MacOs = 2,
    WindowsWtf8 = 3,
}

/// Comparison behavior selected at workspace registration.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CaseSensitivityMode {
    Sensitive,
    Insensitive,
}

/// Reversible canonical workspace-relative path and its separate display/comparison views.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspacePath {
    pub workspace_id: [u8; 16],
    pub platform_code: PlatformCode,
    pub raw_relative_path_bytes: Vec<u8>,
    pub canonical_component_bytes: Vec<u8>,
    pub comparison_key_bytes: Vec<u8>,
    pub case_sensitivity_mode: CaseSensitivityMode,
    pub display_string: String,
    pub display_is_lossy: bool,
}

fn encode_component(component: &[u8]) -> (Vec<u8>, bool) {
    if let Ok(text) = std::str::from_utf8(component) {
        let mut encoded = Vec::new();
        for byte in text.as_bytes() {
            match byte {
                b'/' | b'%' | 0..=31 | 127 => {
                    encoded.extend(format!("%{byte:02X}").as_bytes());
                }
                _ => encoded.push(*byte),
            }
        }
        (encoded, false)
    } else {
        let mut encoded = Vec::new();
        for byte in component {
            if byte.is_ascii_graphic() && !matches!(byte, b'/' | b'%') {
                encoded.push(*byte);
            } else {
                encoded.extend(format!("%{byte:02X}").as_bytes());
            }
        }
        (encoded, true)
    }
}

fn decode_component(component: &[u8]) -> Result<Vec<u8>, IdentityError> {
    let mut decoded = Vec::new();
    let mut cursor = 0;
    while cursor < component.len() {
        if component[cursor] == b'%' {
            let pair = component
                .get(cursor + 1..cursor + 3)
                .ok_or(IdentityError::PathEncoding)?;
            if !pair
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'A'..=b'F'))
            {
                return Err(IdentityError::PathEncoding);
            }
            decoded.push(
                u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
                    .map_err(|_| IdentityError::PathEncoding)?,
            );
            cursor += 3;
        } else {
            decoded.push(component[cursor]);
            cursor += 1;
        }
    }
    Ok(decoded)
}

impl WorkspacePath {
    /// Build all path views from authoritative raw components without symlink resolution.
    ///
    /// # Errors
    ///
    /// Returns an error for a reserved platform or non-reversible internal encoding.
    pub fn from_components(
        workspace_id: [u8; 16],
        platform_code: PlatformCode,
        case_sensitivity_mode: CaseSensitivityMode,
        components: &[Vec<u8>],
    ) -> Result<Self, IdentityError> {
        if platform_code == PlatformCode::WindowsWtf8 {
            return Err(IdentityError::Platform(platform_code as u8));
        }
        let raw_relative_path_bytes =
            components
                .iter()
                .enumerate()
                .fold(Vec::new(), |mut path, (index, component)| {
                    if index > 0 {
                        path.push(b'/');
                    }
                    path.extend(component);
                    path
                });
        let mut lossy = false;
        let canonical_components = components
            .iter()
            .map(|component| {
                let (encoded, component_lossy) = encode_component(component);
                lossy |= component_lossy;
                encoded
            })
            .collect::<Vec<_>>();
        let canonical_component_bytes = canonical_components.join(&b'/');
        let mut display_string = String::new();
        for (index, component) in components.iter().enumerate() {
            if index > 0 {
                display_string.push('/');
            }
            if let Ok(text) = std::str::from_utf8(component) {
                display_string.push_str(text);
            } else {
                let encoded = encode_component(component).0;
                display_string.push_str(
                    std::str::from_utf8(&encoded).map_err(|_| IdentityError::PathEncoding)?,
                );
            }
        }
        let comparison_key_bytes = if platform_code == PlatformCode::MacOs
            && case_sensitivity_mode == CaseSensitivityMode::Insensitive
        {
            if components
                .iter()
                .all(|component| std::str::from_utf8(component).is_ok())
            {
                let text = components
                    .iter()
                    .map(|component| {
                        std::str::from_utf8(component).map_err(|_| IdentityError::PathEncoding)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join("/");
                text.nfd().case_fold().collect::<String>().into_bytes()
            } else {
                lossy = true;
                raw_relative_path_bytes.clone()
            }
        } else {
            raw_relative_path_bytes.clone()
        };
        Ok(Self {
            workspace_id,
            platform_code,
            raw_relative_path_bytes,
            canonical_component_bytes,
            comparison_key_bytes,
            case_sensitivity_mode,
            display_string,
            display_is_lossy: lossy,
        })
    }

    /// Decode canonical component bytes back to exact component bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or lowercase percent escapes.
    pub fn decoded_components(&self) -> Result<Vec<Vec<u8>>, IdentityError> {
        self.canonical_component_bytes
            .split(|byte| *byte == b'/')
            .map(|component| {
                let decoded = decode_component(component)?;
                if encode_component(&decoded).0 != component {
                    return Err(IdentityError::PathEncoding);
                }
                Ok(decoded)
            })
            .collect()
    }

    /// Canonical URI with raw path bytes encoded as unpadded base64url.
    #[must_use]
    pub fn canonical_uri(&self) -> String {
        format!(
            "codefabric://workspace/{}/path/{}",
            lower_hex(&self.workspace_id),
            URL_SAFE_NO_PAD.encode(&self.raw_relative_path_bytes)
        )
    }

    /// Total deterministic ordering key.
    #[must_use]
    pub fn ordering_key(&self) -> (&[u8], &[u8]) {
        (&self.comparison_key_bytes, &self.raw_relative_path_bytes)
    }
}

/// Probe the selected filesystem volume by a reversible create/alternate-case lookup.
///
/// # Errors
///
/// Returns an error when the probe file cannot be created, inspected, or removed.
pub fn probe_case_sensitivity(directory: &Path) -> Result<CaseSensitivityMode, IdentityError> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| IdentityError::CaseProbe(error.to_string()))?
        .as_nanos();
    let lower = directory.join(format!(".codefabric-case-probe-{suffix:x}"));
    let upper = directory.join(format!(".CODEFABRIC-CASE-PROBE-{suffix:X}"));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lower)
        .map_err(|error| IdentityError::CaseProbe(error.to_string()))?;
    drop(file);
    let insensitive = upper.exists();
    std::fs::remove_file(&lower).map_err(|error| IdentityError::CaseProbe(error.to_string()))?;
    Ok(if insensitive {
        CaseSensitivityMode::Insensitive
    } else {
        CaseSensitivityMode::Sensitive
    })
}

/// Closed AC-G-15 constructor set.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u16)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TypeConstructor {
    Unknown,
    Error,
    AnyDynamic,
    NeverBottom,
    NullNone,
    Primitive,
    Nominal,
    Alias,
    Literal,
    Union,
    Intersection,
    Tuple,
    Callable,
    TypeObject,
    ClassObject,
    Generic,
    TypeVariable,
    AssociatedType,
    Projection,
    Reference,
    RawPointer,
    Array,
    Slice,
    Mapping,
    Sequence,
    Structural,
    FunctionDefinition,
    FunctionPointer,
    Closure,
    Coroutine,
    DynTrait,
    ImplTrait,
    ConstArgument,
    RecursiveBinder,
    BoundVariable,
}

impl TypeConstructor {
    /// Return the one-based constructor code owned by AC-G-15.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32 + 1
    }
}

/// Versioned canonical type term; field tags are constructor-schema owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeTerm {
    pub constructor: TypeConstructor,
    pub fields: Vec<CbefField>,
}

/// Provider-independent type projection accepted from either language adapter.
pub trait SemanticTypeAdapter<Observation> {
    type Error;

    /// Normalize one provider observation into the application-owned type algebra.
    ///
    /// # Errors
    ///
    /// Returns the adapter's bounded normalization failure.
    fn normalize(&self, observation: &Observation) -> Result<TypeTerm, Self::Error>;
}

/// Canonical type identity and storage key derived solely from one normalized term.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InternedType {
    pub type_id: [u8; 16],
    pub type_kind_code: i32,
    pub canonical_key: String,
}

impl TypeTerm {
    /// Normalize set-like type members and encode one canonical constructor record.
    ///
    /// # Errors
    ///
    /// Returns an error when a constructor field is not canonically encodable.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, IdentityError> {
        let mut fields = self.fields.clone();
        if matches!(
            self.constructor,
            TypeConstructor::Union | TypeConstructor::Intersection
        ) {
            for field in &mut fields {
                if let CbefValue::OrderedList(values) = &field.value {
                    field.value = CbefValue::Set(values.clone());
                }
            }
        }
        let mut encoded = vec![0x01];
        put_u16(&mut encoded, self.constructor as u16 + 1);
        encoded.extend(encode_record(&CbefRecord {
            domain: IdentityDomain::Type,
            fields,
        })?);
        Ok(encoded)
    }
}

/// Shape-checked, idempotent type interner using CBEF-derived type IDs.
#[derive(Clone, Debug, Default)]
pub struct TypeInterner {
    by_id: BTreeMap<[u8; 16], Vec<u8>>,
}

impl TypeInterner {
    /// Intern a type in its workspace/context; identical normalized shapes return one ID.
    ///
    /// # Errors
    ///
    /// Returns an encoding error or `ID_COLLISION` for unequal terms sharing an ID.
    pub fn intern(
        &mut self,
        workspace_id: [u8; 16],
        context_id: [u8; 16],
        term: &TypeTerm,
    ) -> Result<[u8; 16], IdentityError> {
        let shape = term.canonical_bytes()?;
        let identity = derive_identity(&CbefRecord {
            domain: IdentityDomain::Type,
            fields: vec![
                CbefField {
                    tag: 1,
                    value: CbefValue::Id(workspace_id),
                },
                CbefField {
                    tag: 2,
                    value: CbefValue::Id(context_id),
                },
                CbefField {
                    tag: 3,
                    value: CbefValue::Unsigned(vec![0, 1]),
                },
                CbefField {
                    tag: 4,
                    value: CbefValue::Bytes(shape.clone()),
                },
            ],
        })?;
        if let Some(existing) = self.by_id.get(&identity.id) {
            if existing != &shape {
                return Err(IdentityError::IdCollision {
                    id: lower_hex(&identity.id),
                });
            }
        } else {
            self.by_id.insert(identity.id, shape);
        }
        Ok(identity.id)
    }

    /// Normalize a provider observation through an application-owned adapter and intern it.
    ///
    /// The returned storage key is the reversible base64url encoding of the canonical AC-G-15
    /// term bytes. Provider display/debug strings never participate in identity.
    ///
    /// # Errors
    ///
    /// Returns the adapter failure separately from canonical identity failures.
    pub fn intern_observation<Observation, Adapter>(
        &mut self,
        workspace_id: [u8; 16],
        context_id: [u8; 16],
        adapter: &Adapter,
        observation: &Observation,
    ) -> Result<InternedType, TypeAdapterError<Adapter::Error>>
    where
        Adapter: SemanticTypeAdapter<Observation>,
    {
        let term = adapter
            .normalize(observation)
            .map_err(TypeAdapterError::Adapter)?;
        let canonical = term.canonical_bytes()?;
        let type_id = self.intern(workspace_id, context_id, &term)?;
        Ok(InternedType {
            type_id,
            type_kind_code: term.constructor.code(),
            canonical_key: format!(
                "cbef-type-v1:{}",
                URL_SAFE_NO_PAD.encode(canonical.as_slice())
            ),
        })
    }
}

/// Failure while adapting and interning a language-provider type observation.
#[derive(Debug, Error)]
pub enum TypeAdapterError<AdapterError> {
    #[error("semantic type adapter rejected the provider observation: {0}")]
    Adapter(AdapterError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

/// Reject comparison-key collisions while preserving deterministic ordering.
///
/// # Errors
///
/// Returns `PathCollision` when distinct raw paths share one comparison key.
pub fn validate_workspace_paths(paths: &[WorkspacePath]) -> Result<(), IdentityError> {
    let mut seen = BTreeMap::<&[u8], &[u8]>::new();
    for path in paths {
        if let Some(raw) = seen.insert(&path.comparison_key_bytes, &path.raw_relative_path_bytes)
            && raw != path.raw_relative_path_bytes
        {
            return Err(IdentityError::PathCollision);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Deserialize)]
    struct JsonField {
        tag: u16,
        value: JsonTypedValue,
    }

    #[derive(Deserialize)]
    struct JsonTypedValue {
        type_code: u8,
        value: Value,
        normalization: Option<String>,
        platform_code: Option<u8>,
    }

    #[derive(Deserialize)]
    struct CbefCase {
        domain_code: u16,
        kind_slug: Option<String>,
        fields: Vec<JsonField>,
        expected_preimage_hex: String,
        expected_digest_hex: String,
        expected_id_hex: String,
        expected_public_id: String,
    }

    #[derive(Deserialize)]
    struct RawCbefCase {
        domain_code: u16,
        fields: Vec<JsonField>,
        expected_preimage_hex: String,
        expected_digest_hex: String,
    }

    #[derive(Deserialize)]
    struct CbefFixture {
        cases: Vec<CbefCase>,
        all_type_codes: RawCbefCase,
    }

    #[derive(Deserialize)]
    struct PathCase {
        id: String,
        workspace_id_hex: String,
        platform_code: u8,
        case_sensitivity_mode: String,
        components_hex: Vec<String>,
        expected_raw_hex: String,
        expected_canonical_hex: String,
        expected_comparison_hex: String,
        expected_display: String,
        expected_display_is_lossy: bool,
        expected_uri: String,
    }

    #[derive(Deserialize)]
    struct PathFixture {
        cases: Vec<PathCase>,
        collision_pairs: Vec<[String; 2]>,
    }

    #[derive(Deserialize)]
    struct TypeCase {
        constructor_code: usize,
        fields: Vec<JsonField>,
        workspace_id_hex: String,
        analysis_context_id_hex: String,
        expected_canonical_term_hex: String,
        expected_identity_preimage_hex: String,
        expected_type_digest_hex: String,
        expected_type_id_hex: String,
    }

    #[derive(Deserialize)]
    struct TypeFixture {
        cases: Vec<TypeCase>,
    }

    fn hex(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0);
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn fixed<const N: usize>(value: &str) -> [u8; N] {
        hex(value).try_into().unwrap()
    }

    fn json_value(value: &JsonTypedValue) -> CbefValue {
        match CbefTypeCode::from_code(value.type_code).unwrap() {
            CbefTypeCode::Absent => CbefValue::Absent,
            CbefTypeCode::Bytes => CbefValue::Bytes(hex(value.value.as_str().unwrap())),
            CbefTypeCode::Utf8 => CbefValue::Utf8 {
                value: value.value.as_str().unwrap().to_owned(),
                normalization: match value.normalization.as_deref().unwrap_or("NONE") {
                    "NONE" => StringNormalization::None,
                    "NFC" => StringNormalization::Nfc,
                    "NFKC" => StringNormalization::Nfkc,
                    "ASCII_LOWER" => StringNormalization::AsciiLower,
                    "PYTHON_IDENTIFIER_NFKC" => StringNormalization::PythonIdentifierNfkc,
                    "RUST_CANONICAL" => StringNormalization::RustCanonical,
                    normalization => panic!("unknown normalization {normalization}"),
                },
            },
            CbefTypeCode::RawPath => CbefValue::RawPath {
                platform_code: value.platform_code.unwrap(),
                bytes: hex(value.value.as_str().unwrap()),
            },
            CbefTypeCode::Unsigned => CbefValue::Unsigned(hex(value.value.as_str().unwrap())),
            CbefTypeCode::Signed => CbefValue::Signed(hex(value.value.as_str().unwrap())),
            CbefTypeCode::Boolean => CbefValue::Boolean(value.value.as_bool().unwrap()),
            CbefTypeCode::Id => CbefValue::Id(fixed(value.value.as_str().unwrap())),
            CbefTypeCode::Digest => CbefValue::Digest(fixed(value.value.as_str().unwrap())),
            CbefTypeCode::OrderedList => CbefValue::OrderedList(
                value
                    .value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| json_value(&serde_json::from_value(item.clone()).unwrap()))
                    .collect(),
            ),
            CbefTypeCode::Set => CbefValue::Set(
                value
                    .value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| json_value(&serde_json::from_value(item.clone()).unwrap()))
                    .collect(),
            ),
            CbefTypeCode::Map => CbefValue::Map(
                value
                    .value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| {
                        let entry = entry.as_object().unwrap();
                        (
                            json_value(&serde_json::from_value(entry["key"].clone()).unwrap()),
                            json_value(&serde_json::from_value(entry["value"].clone()).unwrap()),
                        )
                    })
                    .collect(),
            ),
            CbefTypeCode::TaggedUnion => {
                let tagged = value.value.as_object().unwrap();
                CbefValue::TaggedUnion {
                    variant: u16::try_from(tagged["variant"].as_u64().unwrap()).unwrap(),
                    value: Box::new(json_value(
                        &serde_json::from_value(tagged["value"].clone()).unwrap(),
                    )),
                }
            }
        }
    }

    fn fields(values: &[JsonField]) -> Vec<CbefField> {
        values
            .iter()
            .map(|field| CbefField {
                tag: field.tag,
                value: json_value(&field.value),
            })
            .collect()
    }

    const TYPE_CONSTRUCTORS: [TypeConstructor; 35] = [
        TypeConstructor::Unknown,
        TypeConstructor::Error,
        TypeConstructor::AnyDynamic,
        TypeConstructor::NeverBottom,
        TypeConstructor::NullNone,
        TypeConstructor::Primitive,
        TypeConstructor::Nominal,
        TypeConstructor::Alias,
        TypeConstructor::Literal,
        TypeConstructor::Union,
        TypeConstructor::Intersection,
        TypeConstructor::Tuple,
        TypeConstructor::Callable,
        TypeConstructor::TypeObject,
        TypeConstructor::ClassObject,
        TypeConstructor::Generic,
        TypeConstructor::TypeVariable,
        TypeConstructor::AssociatedType,
        TypeConstructor::Projection,
        TypeConstructor::Reference,
        TypeConstructor::RawPointer,
        TypeConstructor::Array,
        TypeConstructor::Slice,
        TypeConstructor::Mapping,
        TypeConstructor::Sequence,
        TypeConstructor::Structural,
        TypeConstructor::FunctionDefinition,
        TypeConstructor::FunctionPointer,
        TypeConstructor::Closure,
        TypeConstructor::Coroutine,
        TypeConstructor::DynTrait,
        TypeConstructor::ImplTrait,
        TypeConstructor::ConstArgument,
        TypeConstructor::RecursiveBinder,
        TypeConstructor::BoundVariable,
    ];

    fn sample_record() -> CbefRecord {
        CbefRecord {
            domain: IdentityDomain::Workspace,
            fields: vec![
                CbefField {
                    tag: 1,
                    value: CbefValue::Bytes(vec![0x11; 16]),
                },
                CbefField {
                    tag: 2,
                    value: CbefValue::Utf8 {
                        value: "Directory".to_owned(),
                        normalization: StringNormalization::AsciiLower,
                    },
                },
            ],
        }
    }

    #[test]
    fn wp55_behavioral_acceptance() {
        let workspace_id = [0x11; 16];
        let analysis_context_id = [0x22; 16];
        let owner_id = [0x33; 16];
        let provider_run_id = [0x44; 16];
        let observation_id = [0x55; 16];
        let fact_id = [0x66; 16];

        let mut legacy_capability = blake3::Hasher::new();
        legacy_capability.update(b"codefabric-capability-scope-v1\0");
        legacy_capability.update(&workspace_id);
        legacy_capability.update(&analysis_context_id);
        legacy_capability.update(&owner_id);
        legacy_capability.update(&7_i64.to_be_bytes());
        legacy_capability.update(b"SEMANTIC_TYPES");
        assert_eq!(
            capability_scope_fingerprint(
                workspace_id,
                analysis_context_id,
                owner_id,
                7,
                "SEMANTIC_TYPES",
            ),
            *legacy_capability.finalize().as_bytes()
        );

        let mut legacy_evidence = blake3::Hasher::new();
        legacy_evidence.update(b"codefabric-fact-evidence-v1\0");
        legacy_evidence.update(&provider_run_id);
        legacy_evidence.update(&observation_id);
        legacy_evidence.update(&fact_id);
        assert_eq!(
            fact_evidence_id(provider_run_id, observation_id, fact_id),
            legacy_evidence.finalize().as_bytes()[..16]
        );

        let mut legacy_observation = blake3::Hasher::new();
        legacy_observation.update(b"codefabric-source-observation-v1\0");
        legacy_observation.update(&provider_run_id);
        legacy_observation.update(&[9]);
        legacy_observation.update(&42_u64.to_be_bytes());
        assert_eq!(
            source_observation_id(provider_run_id, 9, 42),
            legacy_observation.finalize().as_bytes()[..16]
        );

        let uppercase =
            semantic_owner_identity(workspace_id, analysis_context_id, "MIR-BODY", vec![0xaa])
                .unwrap();
        let normalized =
            semantic_owner_identity(workspace_id, analysis_context_id, "mir-body", vec![0xaa])
                .unwrap();
        assert_eq!(uppercase, normalized);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn wp07_behavioral_acceptance() {
        let cbef: CbefFixture = serde_json::from_str(include_str!(
            "../contracts/fixtures/identity/cbef-v1-vectors.json"
        ))
        .unwrap();
        for case in cbef.cases {
            let record = CbefRecord {
                domain: IdentityDomain::from_code(case.domain_code).unwrap(),
                fields: fields(&case.fields),
            };
            let identity = derive_identity(&record).unwrap();
            assert_eq!(identity.preimage, hex(&case.expected_preimage_hex));
            assert_eq!(identity.full_digest, fixed(&case.expected_digest_hex));
            assert_eq!(identity.id, fixed(&case.expected_id_hex));
            assert_eq!(
                encode_public_id(record.domain, case.kind_slug.as_deref(), identity.id).unwrap(),
                case.expected_public_id
            );
        }
        let all_types = CbefRecord {
            domain: IdentityDomain::from_code(cbef.all_type_codes.domain_code).unwrap(),
            fields: fields(&cbef.all_type_codes.fields),
        };
        let all_types = derive_identity(&all_types).unwrap();
        assert_eq!(
            all_types.preimage,
            hex(&cbef.all_type_codes.expected_preimage_hex)
        );
        assert_eq!(
            all_types.full_digest,
            fixed(&cbef.all_type_codes.expected_digest_hex)
        );

        let paths: PathFixture = serde_json::from_str(include_str!(
            "../contracts/fixtures/identity/path-canonicalization-v1-vectors.json"
        ))
        .unwrap();
        let mut by_id = BTreeMap::new();
        for case in paths.cases {
            let path = WorkspacePath::from_components(
                fixed(&case.workspace_id_hex),
                match case.platform_code {
                    1 => PlatformCode::Unix,
                    2 => PlatformCode::MacOs,
                    value => panic!("unexpected platform {value}"),
                },
                match case.case_sensitivity_mode.as_str() {
                    "sensitive" => CaseSensitivityMode::Sensitive,
                    "insensitive" => CaseSensitivityMode::Insensitive,
                    value => panic!("unexpected case mode {value}"),
                },
                &case
                    .components_hex
                    .iter()
                    .map(|value| hex(value))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            assert_eq!(path.raw_relative_path_bytes, hex(&case.expected_raw_hex));
            assert_eq!(
                path.canonical_component_bytes,
                hex(&case.expected_canonical_hex)
            );
            assert_eq!(
                path.comparison_key_bytes,
                hex(&case.expected_comparison_hex)
            );
            assert_eq!(path.display_string, case.expected_display);
            assert_eq!(path.display_is_lossy, case.expected_display_is_lossy);
            assert_eq!(path.canonical_uri(), case.expected_uri);
            assert_eq!(
                path.decoded_components().unwrap(),
                case.components_hex
                    .iter()
                    .map(|value| hex(value))
                    .collect::<Vec<_>>()
            );
            by_id.insert(case.id, path);
        }
        for [left, right] in paths.collision_pairs {
            assert_eq!(
                validate_workspace_paths(&[by_id[&left].clone(), by_id[&right].clone()]),
                Err(IdentityError::PathCollision)
            );
        }

        let types: TypeFixture = serde_json::from_str(include_str!(
            "../contracts/fixtures/identity/type-algebra-v1-vectors.json"
        ))
        .unwrap();
        for case in types.cases {
            let term = TypeTerm {
                constructor: TYPE_CONSTRUCTORS[case.constructor_code - 1],
                fields: fields(&case.fields),
            };
            assert_eq!(
                term.canonical_bytes().unwrap(),
                hex(&case.expected_canonical_term_hex)
            );
            let mut interner = TypeInterner::default();
            let type_id = interner
                .intern(
                    fixed(&case.workspace_id_hex),
                    fixed(&case.analysis_context_id_hex),
                    &term,
                )
                .unwrap();
            assert_eq!(type_id, fixed(&case.expected_type_id_hex));
            let identity = derive_identity(&CbefRecord {
                domain: IdentityDomain::Type,
                fields: vec![
                    CbefField {
                        tag: 1,
                        value: CbefValue::Id(fixed(&case.workspace_id_hex)),
                    },
                    CbefField {
                        tag: 2,
                        value: CbefValue::Id(fixed(&case.analysis_context_id_hex)),
                    },
                    CbefField {
                        tag: 3,
                        value: CbefValue::Unsigned(vec![0, 1]),
                    },
                    CbefField {
                        tag: 4,
                        value: CbefValue::Bytes(term.canonical_bytes().unwrap()),
                    },
                ],
            })
            .unwrap();
            assert_eq!(identity.preimage, hex(&case.expected_identity_preimage_hex));
            assert_eq!(identity.full_digest, fixed(&case.expected_type_digest_hex));
        }

        let encoded = encode_record(&sample_record()).unwrap();
        let decoded = decode_record(&encoded).unwrap();
        assert_eq!(decoded.domain, IdentityDomain::Workspace);
        assert_eq!(derive_identity(&sample_record()).unwrap().id.len(), 16);

        let public =
            encode_public_id(IdentityDomain::Entity, Some("callable"), [0xab; 16]).unwrap();
        assert_eq!(
            decode_public_id(IdentityDomain::Entity, Some("callable"), &public).unwrap(),
            [0xab; 16]
        );

        let path = WorkspacePath::from_components(
            [0x22; 16],
            PlatformCode::Unix,
            CaseSensitivityMode::Sensitive,
            &[b"src".to_vec(), b"a%b".to_vec(), vec![0xff]],
        )
        .unwrap();
        assert_eq!(
            path.decoded_components().unwrap(),
            [b"src".to_vec(), b"a%b".to_vec(), vec![0xff]]
        );

        let term = TypeTerm {
            constructor: TypeConstructor::Union,
            fields: vec![CbefField {
                tag: 1,
                value: CbefValue::OrderedList(vec![
                    CbefValue::Id([2; 16]),
                    CbefValue::Id([1; 16]),
                    CbefValue::Id([2; 16]),
                ]),
            }],
        };
        let mut interner = TypeInterner::default();
        let first = interner.intern([3; 16], [4; 16], &term).unwrap();
        assert_eq!(first, interner.intern([3; 16], [4; 16], &term).unwrap());
    }

    #[test]
    fn language_type_adapters_consume_one_canonical_type_authority() {
        struct PrimitiveAdapter;

        impl SemanticTypeAdapter<&str> for PrimitiveAdapter {
            type Error = std::convert::Infallible;

            fn normalize(&self, observation: &&str) -> Result<TypeTerm, Self::Error> {
                Ok(TypeTerm {
                    constructor: TypeConstructor::Primitive,
                    fields: vec![CbefField {
                        tag: 1,
                        value: CbefValue::Utf8 {
                            value: (*observation).to_owned(),
                            normalization: StringNormalization::AsciiLower,
                        },
                    }],
                })
            }
        }

        let mut interner = TypeInterner::default();
        let first = interner
            .intern_observation([1; 16], [2; 16], &PrimitiveAdapter, &"INTEGER")
            .unwrap();
        let second = interner
            .intern_observation([1; 16], [2; 16], &PrimitiveAdapter, &"integer")
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.type_kind_code, TypeConstructor::Primitive.code());
        assert!(first.canonical_key.starts_with("cbef-type-v1:"));

        let different_context = interner
            .intern_observation([1; 16], [3; 16], &PrimitiveAdapter, &"integer")
            .unwrap();
        assert_ne!(first.type_id, different_context.type_id);
        assert_eq!(first.canonical_key, different_context.canonical_key);
    }

    #[test]
    fn wp07_structural_acceptance() {
        assert_eq!(IdentityDomain::UnknownRemainder as u16, 16);
        assert_eq!(IdentityDomain::RootAuthorization as u16, 17);
        assert_eq!(TypeConstructor::BoundVariable as u16 + 1, 35);
        let codes = (0_u8..=12)
            .map(CbefTypeCode::from_code)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(codes.len(), 13);
    }

    #[test]
    fn wp07_negative_zero_state() {
        let mut out_of_order = sample_record();
        out_of_order.fields.reverse();
        assert_eq!(encode_record(&out_of_order), Err(IdentityError::FieldOrder));

        let mut trailing = encode_record(&sample_record()).unwrap();
        trailing.push(0);
        assert_eq!(decode_record(&trailing), Err(IdentityError::TrailingBytes));
        assert!(
            decode_public_id(
                IdentityDomain::Entity,
                Some("callable"),
                "entity:Callable:00"
            )
            .is_err()
        );

        let first = derive_identity(&sample_record()).unwrap();
        let mut collision = first.clone();
        collision.full_digest[31] ^= 1;
        collision.preimage.push(0);
        let mut registry = IdentityRegistry::default();
        registry.register(&first).unwrap();
        assert!(matches!(
            registry.register(&collision),
            Err(IdentityError::IdCollision { .. })
        ));
    }

    #[test]
    fn wp07_operational_acceptance() {
        let first = WorkspacePath::from_components(
            [0; 16],
            PlatformCode::MacOs,
            CaseSensitivityMode::Insensitive,
            &["Straße".as_bytes().to_vec()],
        )
        .unwrap();
        let second = WorkspacePath::from_components(
            [0; 16],
            PlatformCode::MacOs,
            CaseSensitivityMode::Insensitive,
            &["STRASSE".as_bytes().to_vec()],
        )
        .unwrap();
        assert_eq!(first.comparison_key_bytes, second.comparison_key_bytes);
        assert_eq!(
            first.canonical_uri(),
            "codefabric://workspace/00000000000000000000000000000000/path/U3RyYcOfZQ"
        );
        assert!(validate_workspace_paths(&[first, second]).is_err());
    }
}
