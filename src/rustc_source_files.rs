//! Captured application source identities shared with the compiler subprocess.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedRustSourceFile {
    pub file_id: String,
    pub content_digest: [u8; 32],
}

/// Keys are exact workspace-relative path bytes; identities come from source capture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustSourceFileManifest {
    pub workspace_id: String,
    pub source_generation: u64,
    #[serde(with = "paths")]
    pub files: BTreeMap<Vec<u8>, CapturedRustSourceFile>,
}

impl RustSourceFileManifest {
    /// Decode a bounded manifest without allowing path aliases or duplicate file identities.
    ///
    /// # Errors
    /// Rejects malformed JSON, an oversized inventory, invalid identities and aliased paths.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err("compiler source manifest exceeds its byte limit".into());
        }
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid compiler source manifest: {error}"))?;
        if manifest.files.is_empty() || manifest.files.len() > 1_000_000 {
            return Err("compiler source inventory is empty or oversized".into());
        }
        let mut identities = BTreeSet::new();
        for (path, file) in &manifest.files {
            if path.is_empty()
                || path.contains(&0)
                || path.len() > 16_384
                || path
                    .split(|byte| *byte == b'/')
                    .any(|part| part.is_empty() || part == b"." || part == b"..")
                || !valid_id(&file.file_id, "file:")
                || !identities.insert(&file.file_id)
            {
                return Err("compiler source manifest contains an invalid path or identity".into());
            }
        }
        if !valid_id(&manifest.workspace_id, "workspace:") {
            return Err("compiler source manifest contains an invalid workspace identity".into());
        }
        Ok(manifest)
    }
}

/// New manifests use explicit byte paths. The old UTF-8 map remains readable for
/// retained immutable views; neither representation permits duplicate keys.
mod paths {
    use super::{BTreeMap, CapturedRustSourceFile};
    use serde::de::{Error as _, MapAccess, SeqAccess, Visitor};
    use serde::ser::SerializeSeq as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Entry<P, F> {
        raw_relative_path_bytes: P,
        file: F,
    }

    pub(super) fn serialize<S: Serializer>(
        files: &BTreeMap<Vec<u8>, CapturedRustSourceFile>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_seq(Some(files.len()))?;
        for (path, file) in files {
            output.serialize_element(&Entry {
                raw_relative_path_bytes: path,
                file,
            })?;
        }
        output.end()
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<Vec<u8>, CapturedRustSourceFile>, D::Error> {
        struct Paths;
        impl<'de> Visitor<'de> for Paths {
            type Value = BTreeMap<Vec<u8>, CapturedRustSourceFile>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("byte-path entries or the legacy UTF-8 source map")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
                let mut files = BTreeMap::new();
                while let Some(entry) =
                    input.next_element::<Entry<Vec<u8>, CapturedRustSourceFile>>()?
                {
                    if files.len() == 1_000_000
                        || files
                            .insert(entry.raw_relative_path_bytes, entry.file)
                            .is_some()
                    {
                        return Err(A::Error::custom(
                            "duplicate or excessive compiler source paths",
                        ));
                    }
                }
                Ok(files)
            }

            fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
                let mut files = BTreeMap::new();
                while let Some((path, file)) =
                    input.next_entry::<String, CapturedRustSourceFile>()?
                {
                    if files.len() == 1_000_000 || files.insert(path.into_bytes(), file).is_some() {
                        return Err(A::Error::custom(
                            "duplicate or excessive compiler source paths",
                        ));
                    }
                }
                Ok(files)
            }
        }
        deserializer.deserialize_any(Paths)
    }
}

fn valid_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|id| {
        id.len() == 32
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(byte: u8) -> CapturedRustSourceFile {
        CapturedRustSourceFile {
            file_id: format!("file:{}", format!("{byte:02x}").repeat(16)),
            content_digest: [byte; 32],
        }
    }

    fn manifest() -> RustSourceFileManifest {
        RustSourceFileManifest {
            workspace_id: format!("workspace:{}", "01".repeat(16)),
            source_generation: 7,
            files: BTreeMap::from([
                (b"src/lib.rs".to_vec(), file(1)),
                (b"pkg/bad_\xff.py".to_vec(), file(2)),
                ("pkg/bad_�.py".as_bytes().to_vec(), file(3)),
            ]),
        }
    }

    #[test]
    fn compiler_source_manifest_retains_raw_paths_and_reads_legacy_utf8() {
        let expected = manifest();
        let encoded = serde_json::to_vec(&expected).unwrap();
        let decoded = RustSourceFileManifest::decode(&encoded).unwrap();
        assert_eq!(decoded, expected);
        assert_ne!(
            decoded.files[b"pkg/bad_\xff.py".as_slice()],
            decoded.files["pkg/bad_�.py".as_bytes()]
        );
        let legacy = serde_json::json!({
            "workspace_id": expected.workspace_id,
            "source_generation": 7,
            "files": {"src/lib.rs": file(1)}
        });
        let decoded =
            RustSourceFileManifest::decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(
            decoded.files,
            BTreeMap::from([(b"src/lib.rs".to_vec(), file(1))])
        );
    }

    #[test]
    fn compiler_source_manifest_rejects_duplicate_keys_and_aliased_raw_paths() {
        let original = manifest();
        let mut wire = serde_json::to_value(&original).unwrap();
        let duplicate = wire["files"][0].clone();
        wire["files"].as_array_mut().unwrap().push(duplicate);
        assert!(RustSourceFileManifest::decode(&serde_json::to_vec(&wire).unwrap()).is_err());
        let legacy = format!(
            "{{\"workspace_id\":\"{}\",\"source_generation\":7,\"files\":{{\"a.rs\":{},\"a.rs\":{}}}}}",
            original.workspace_id,
            serde_json::to_string(&file(1)).unwrap(),
            serde_json::to_string(&file(2)).unwrap()
        );
        assert!(RustSourceFileManifest::decode(legacy.as_bytes()).is_err());
        for invalid in [
            b"".as_slice(),
            b"/a.rs",
            b"a//b.rs",
            b"a/../b.rs",
            b"a/./b.rs",
            b"a/",
            b"a\0.rs",
        ] {
            let mut candidate = original.clone();
            candidate.files = BTreeMap::from([(invalid.to_vec(), file(1))]);
            assert!(
                RustSourceFileManifest::decode(&serde_json::to_vec(&candidate).unwrap()).is_err(),
                "{invalid:?}"
            );
        }
        let mut duplicate_identity = original;
        duplicate_identity
            .files
            .insert(b"another.rs".to_vec(), file(1));
        assert!(
            RustSourceFileManifest::decode(&serde_json::to_vec(&duplicate_identity).unwrap())
                .is_err()
        );
    }
}
