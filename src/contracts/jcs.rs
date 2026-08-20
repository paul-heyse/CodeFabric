//! `codefabric-jcs-v1`: RFC 8785 plus the restrictions owned by Query AC-G-53.

use std::collections::HashSet;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor};
use serde_json::{Number, Value};
use thiserror::Error;

/// Canonical JSON profile implemented by this module.
pub const PROFILE: &str = "codefabric-jcs-v1";

/// Largest positive or negative integer exactly interoperable with IEEE-754 binary64.
pub const MAX_SAFE_INTEGER: i128 = 9_007_199_254_740_991;

/// A canonical JSON boundary violation.
#[derive(Debug, Error)]
pub enum CanonicalJsonError {
    /// JSON decoding failed before canonicalization.
    #[error("invalid JSON: {0}")]
    InvalidJson(#[source] serde_json::Error),
    /// A JavaScript non-finite numeric token appeared outside a JSON string.
    #[error("invalid non-finite JSON number token: {0}")]
    InvalidJsonNumber(String),
    /// An object repeated a member name.
    #[error("duplicate object key: {0}")]
    DuplicateKey(String),
    /// A JSON integer was not in the exact interoperable range.
    #[error("integer outside the interoperable JSON range: {0}")]
    IntegerOutOfRange(String),
    /// A number could not be represented as a finite binary64 value.
    #[error("non-finite or unrepresentable JSON number: {0}")]
    InvalidNumber(String),
    /// The adopted RFC 8785 serializer rejected the value.
    #[error("RFC 8785 serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    /// A schema-owned string format was not in canonical form.
    #[error("invalid {format} value: {value}")]
    InvalidFormat {
        /// JSON Schema format name.
        format: &'static str,
        /// Rejected value.
        value: String,
    },
    /// A non-string map contained the same canonical key twice.
    #[error("duplicate canonical non-string map key")]
    DuplicateCanonicalKey,
}

impl CanonicalJsonError {
    /// Return the stable conformance class used by the shared fixture corpus.
    #[must_use]
    pub const fn failure_class(&self) -> &'static str {
        match self {
            Self::InvalidJson(_) => "invalid-json",
            Self::InvalidJsonNumber(_) => "invalid-json-number",
            Self::DuplicateKey(_) => "duplicate-key",
            Self::IntegerOutOfRange(_) => "integer-range",
            Self::InvalidNumber(_) => "finite-number",
            Self::Serialization(_) => "serialization",
            Self::InvalidFormat { format, .. } => format,
            Self::DuplicateCanonicalKey => "duplicate-canonical-key",
        }
    }
}

#[derive(Debug)]
struct NoDuplicateKeys;

impl<'de> Deserialize<'de> for NoDuplicateKeys {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(NoDuplicateVisitor)
    }
}

struct NoDuplicateVisitor;

impl<'de> Visitor<'de> for NoDuplicateVisitor {
    type Value = NoDuplicateKeys;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        NoDuplicateKeys::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<NoDuplicateKeys>()?.is_some() {}
        Ok(NoDuplicateKeys)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(A::Error::custom(format!("CODEFABRIC_DUPLICATE_KEY:{key}")));
            }
            map.next_value::<NoDuplicateKeys>()?;
        }
        Ok(NoDuplicateKeys)
    }
}

fn map_duplicate_error(error: serde_json::Error) -> CanonicalJsonError {
    let rendered = error.to_string();
    if let Some(rest) = rendered.strip_prefix("CODEFABRIC_DUPLICATE_KEY:") {
        let key = rest.split(" at line ").next().unwrap_or(rest).to_owned();
        CanonicalJsonError::DuplicateKey(key)
    } else {
        CanonicalJsonError::InvalidJson(error)
    }
}

fn reject_duplicate_keys(input: &[u8]) -> Result<(), CanonicalJsonError> {
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    NoDuplicateKeys::deserialize(&mut deserializer).map_err(map_duplicate_error)?;
    deserializer.end().map_err(CanonicalJsonError::InvalidJson)
}

fn invalid_non_finite_token(input: &[u8]) -> Option<String> {
    const TOKENS: [&[u8]; 3] = [b"-Infinity", b"Infinity", b"NaN"];

    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < input.len() {
        let byte = input[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }

        for token in TOKENS {
            let end = index.saturating_add(token.len());
            if input.get(index..end) == Some(token)
                && token_boundary_before(input, index)
                && token_boundary_after(input, end)
            {
                return Some(String::from_utf8_lossy(token).into_owned());
            }
        }
        index += 1;
    }
    None
}

fn token_boundary_before(input: &[u8], index: usize) -> bool {
    index == 0
        || input[index - 1].is_ascii_whitespace()
        || matches!(input[index - 1], b'[' | b'{' | b':' | b',')
}

fn token_boundary_after(input: &[u8], index: usize) -> bool {
    index == input.len()
        || input[index].is_ascii_whitespace()
        || matches!(input[index], b']' | b'}' | b',')
}

fn validate_number(number: &Number) -> Result<(), CanonicalJsonError> {
    let rendered = number.to_string();
    if rendered.contains(['.', 'e', 'E']) {
        return number
            .as_f64()
            .filter(|value| value.is_finite())
            .map(|_| ())
            .ok_or(CanonicalJsonError::InvalidNumber(rendered));
    }

    let integer = rendered
        .parse::<i128>()
        .map_err(|_| CanonicalJsonError::IntegerOutOfRange(rendered.clone()))?;
    if (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&integer) {
        Ok(())
    } else {
        Err(CanonicalJsonError::IntegerOutOfRange(rendered))
    }
}

fn validate_value(value: &Value) -> Result<(), CanonicalJsonError> {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
        Value::Number(number) => validate_number(number),
        Value::Array(values) => values.iter().try_for_each(validate_value),
        Value::Object(values) => values.values().try_for_each(validate_value),
    }
}

/// Decode JSON with duplicate detection and emit canonical UTF-8 bytes.
///
/// # Errors
///
/// Returns a typed boundary error for invalid JSON, duplicate keys, unsafe integers,
/// unrepresentable numbers, or an adopted serializer failure.
pub fn canonicalize_slice(input: &[u8]) -> Result<Vec<u8>, CanonicalJsonError> {
    if let Some(token) = invalid_non_finite_token(input) {
        return Err(CanonicalJsonError::InvalidJsonNumber(token));
    }
    reject_duplicate_keys(input)?;
    let value: Value = serde_json::from_slice(input).map_err(CanonicalJsonError::InvalidJson)?;
    canonicalize_value(&value)
}

/// Emit canonical UTF-8 bytes from a validated JSON value.
///
/// # Errors
///
/// Returns an error for unsafe numbers or an adopted serializer failure.
pub fn canonicalize_value(value: &Value) -> Result<Vec<u8>, CanonicalJsonError> {
    validate_value(value)?;
    let canonical =
        serde_json_canonicalizer::to_vec(value).map_err(CanonicalJsonError::Serialization)?;

    // JCS renders numbers through the interoperable binary64 domain. A lexically
    // fractional input can therefore round to an integer token. Re-validate the emitted
    // token domain so every successful result is itself valid codefabric-jcs-v1 input.
    let emitted: Value =
        serde_json::from_slice(&canonical).map_err(CanonicalJsonError::Serialization)?;
    validate_value(&emitted)?;
    Ok(canonical)
}

/// Compute the AC-G-53 BLAKE3-256 checksum form over canonical bytes.
#[must_use]
pub fn checksum(canonical_bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(canonical_bytes).to_hex())
}

/// Validate the exact AC-G-53 BLAKE3-256 checksum frame.
///
/// # Errors
///
/// Returns an error unless the value is `b3:` followed by 64 lowercase hex digits.
pub fn validate_checksum(value: &str) -> Result<(), CanonicalJsonError> {
    let digest = value.strip_prefix("b3:");
    if digest.is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        Ok(())
    } else {
        Err(CanonicalJsonError::InvalidFormat {
            format: "codefabric-checksum",
            value: value.to_owned(),
        })
    }
}

/// Validate a canonical signed 64-bit decimal string.
///
/// # Errors
///
/// Returns an error for leading zeroes, `+`, `-0`, whitespace, or overflow.
pub fn validate_int64(value: &str) -> Result<(), CanonicalJsonError> {
    let parsed = value.parse::<i64>().ok();
    if parsed.is_some_and(|number| number.to_string() == value) {
        Ok(())
    } else {
        Err(CanonicalJsonError::InvalidFormat {
            format: "codefabric-int64",
            value: value.to_owned(),
        })
    }
}

/// Validate a canonical unsigned 64-bit decimal string.
///
/// # Errors
///
/// Returns an error for signs, leading zeroes, whitespace, or overflow.
pub fn validate_uint64(value: &str) -> Result<(), CanonicalJsonError> {
    let parsed = value.parse::<u64>().ok();
    if parsed.is_some_and(|number| number.to_string() == value) {
        Ok(())
    } else {
        Err(CanonicalJsonError::InvalidFormat {
            format: "codefabric-uint64",
            value: value.to_owned(),
        })
    }
}

/// Validate canonical unpadded base64url.
///
/// # Errors
///
/// Returns an error for padding, the standard alphabet, or non-canonical encodings.
pub fn validate_bytes(value: &str) -> Result<(), CanonicalJsonError> {
    let decoded = URL_SAFE_NO_PAD.decode(value).ok();
    if decoded
        .as_ref()
        .is_some_and(|bytes| URL_SAFE_NO_PAD.encode(bytes) == value)
    {
        Ok(())
    } else {
        Err(CanonicalJsonError::InvalidFormat {
            format: "codefabric-bytes",
            value: value.to_owned(),
        })
    }
}

/// Validate a lowercase ASCII public-ID token.
///
/// # Errors
///
/// Returns an error if the token contains non-ASCII or uppercase characters.
pub fn validate_lowercase_public(value: &str) -> Result<(), CanonicalJsonError> {
    if value.is_ascii() && value.bytes().all(|byte| !byte.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err(CanonicalJsonError::InvalidFormat {
            format: "lowercase-public-id",
            value: value.to_owned(),
        })
    }
}

/// Encode a logically non-string-keyed map as canonically sorted key/value records.
///
/// # Errors
///
/// Returns an error when a key cannot be canonicalized or two canonical keys match.
pub fn non_string_map_records(
    entries: impl IntoIterator<Item = (Value, Value)>,
) -> Result<Value, CanonicalJsonError> {
    let mut keyed = entries
        .into_iter()
        .map(|(key, value)| canonicalize_value(&key).map(|bytes| (bytes, key, value)))
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    if keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(CanonicalJsonError::DuplicateCanonicalKey);
    }
    Ok(Value::Array(
        keyed
            .into_iter()
            .map(|(_, key, value)| serde_json::json!({ "key": key, "value": value }))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicates_at_any_depth() {
        let error = canonicalize_slice(br#"{"outer":{"same":1,"same":2}}"#).unwrap_err();
        assert!(matches!(error, CanonicalJsonError::DuplicateKey(key) if key == "same"));
    }

    #[test]
    fn arbitrary_precision_preserves_unsafe_integer_for_rejection() {
        let error = canonicalize_slice(br"184467440737095516160").unwrap_err();
        assert!(matches!(error, CanonicalJsonError::IntegerOutOfRange(_)));
    }

    #[test]
    fn rejects_fractional_input_that_canonicalizes_to_an_unsafe_integer() {
        let error = canonicalize_slice(b"909009254740799291.99")
            .expect_err("canonical output must remain inside the profile domain");
        assert!(matches!(error, CanonicalJsonError::IntegerOutOfRange(_)));
    }

    #[test]
    fn schema_formats_are_strictly_canonical() {
        assert!(validate_int64("-9223372036854775808").is_ok());
        assert!(validate_int64("-0").is_err());
        assert!(validate_uint64("18446744073709551615").is_ok());
        assert!(validate_uint64("01").is_err());
        assert!(validate_bytes("Y29kZWZhYnJpYw").is_ok());
        assert!(validate_bytes("Y29kZWZhYnJpYw==").is_err());
        assert!(validate_lowercase_public("workspace:abcdef0123").is_ok());
        assert!(validate_lowercase_public("Workspace:abcdef0123").is_err());
        assert!(
            validate_checksum(
                "b3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            )
            .is_ok()
        );
        assert!(
            validate_checksum(
                "b3:0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef"
            )
            .is_err()
        );
    }

    #[test]
    fn non_finite_token_scan_ignores_strings_and_reports_stable_class() {
        let canonical = canonicalize_slice(br#"{"value":"NaN and Infinity"}"#).unwrap();
        assert_eq!(canonical, br#"{"value":"NaN and Infinity"}"#);

        let error = canonicalize_slice(br#"{"value":-Infinity}"#).unwrap_err();
        assert_eq!(error.failure_class(), "invalid-json-number");
    }

    #[test]
    fn non_string_map_records_sort_by_canonical_key() {
        let records = non_string_map_records([
            (serde_json::json!(2), serde_json::json!("two")),
            (serde_json::json!(1), serde_json::json!("one")),
        ])
        .unwrap();
        assert_eq!(
            canonicalize_value(&records).unwrap(),
            br#"[{"key":1,"value":"one"},{"key":2,"value":"two"}]"#
        );
    }
}
