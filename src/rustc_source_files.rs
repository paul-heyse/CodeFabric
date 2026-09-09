//! Captured application source identities shared with the compiler subprocess.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

pub const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedRustSourceFile {
    pub file_id: String,
    pub content_digest: [u8; 32],
}

/// Keys are UTF-8 workspace-relative compiler paths; identities come from source capture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustSourceFileManifest {
    pub workspace_id: String,
    pub source_generation: u64,
    pub files: BTreeMap<String, CapturedRustSourceFile>,
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
                || path.contains('\0')
                || path.len() > 16_384
                || !Path::new(path)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
                || Path::new(path)
                    .components()
                    .collect::<std::path::PathBuf>()
                    .to_str()
                    != Some(path)
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

fn valid_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|id| {
        id.len() == 32
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
