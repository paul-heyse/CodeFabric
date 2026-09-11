//! Native Cargo JSON observations. Artifact names and `fresh` are observations, not fact authority.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CargoArtifactTarget {
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    pub src_path: String,
    #[serde(flatten)]
    pub remaining: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CargoArtifactObservation {
    pub package_id: String,
    pub manifest_path: String,
    pub target: CargoArtifactTarget,
    pub profile: BTreeMap<String, serde_json::Value>,
    pub features: Vec<String>,
    pub filenames: Vec<String>,
    pub executable: Option<String>,
    pub fresh: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CargoBuildScriptObservation {
    pub package_id: String,
    pub linked_libs: Vec<String>,
    pub linked_paths: Vec<String>,
    pub cfgs: Vec<String>,
    // Cargo may report cached output even when no build script ran. Preserve duplicate env
    // names and their native order, but never persist their values in the observation tables.
    #[serde(rename(deserialize = "env"), deserialize_with = "environment_digests")]
    pub environment_digests: Vec<(String, String)>,
    pub out_dir: String,
}

fn environment_digests<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<(String, String)>, D::Error> {
    let values = Vec::<(String, String)>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .map(|(name, value)| (name, crate::integrity::framed_digest(value.as_bytes())))
        .collect())
}

#[derive(Deserialize)]
#[serde(tag = "reason")]
enum Message {
    #[serde(rename = "compiler-artifact")]
    Artifact(CargoArtifactObservation),
    #[serde(rename = "build-script-executed")]
    BuildScript(CargoBuildScriptObservation),
    #[serde(rename = "build-finished")]
    Finished { success: bool },
    // Compiler diagnostics have their own bounded, source-bound Arrow provider lane.
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug)]
pub struct CargoOutputObservation {
    pub artifacts: Vec<CargoArtifactObservation>,
    pub build_scripts: Vec<CargoBuildScriptObservation>,
    pub succeeded: bool,
}

impl CargoOutputObservation {
    pub(super) fn parse(bytes: &[u8], succeeded: bool) -> Result<Self, &'static str> {
        let mut artifacts = Vec::new();
        let mut build_scripts = Vec::new();
        let mut finished = None;
        for line in bytes.split(|byte| *byte == b'\n') {
            // Cargo documents mixed stdout; only object lines belong to its message stream.
            if !line.starts_with(b"{") {
                continue;
            }
            let message = serde_json::from_slice::<Message>(line)
                .map_err(|_| "invalid native Cargo message")?;
            match message {
                Message::Finished { success } if finished.is_none() => finished = Some(success),
                Message::Finished { .. } => return Err("duplicate Cargo build terminal"),
                Message::Artifact(artifact) if finished.is_none() => {
                    if artifact.package_id.is_empty()
                        || artifact.manifest_path.is_empty()
                        || artifact.target.name.is_empty()
                        || artifact.target.kind.is_empty()
                        || artifact.target.src_path.is_empty()
                        || artifact.filenames.is_empty()
                    {
                        return Err("incomplete Cargo artifact identity");
                    }
                    artifacts.push(artifact);
                }
                Message::BuildScript(script) if finished.is_none() => {
                    if script.package_id.is_empty() || script.out_dir.is_empty() {
                        return Err("incomplete Cargo build-script observation");
                    }
                    build_scripts.push(script);
                }
                Message::Other => {}
                _ => return Err("Cargo artifact followed its terminal"),
            }
        }
        if finished != Some(succeeded) {
            return Err("Cargo JSON terminal differs from the joined process");
        }
        Ok(Self {
            artifacts,
            build_scripts,
            succeeded,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_and_cached_build_outputs_do_not_imply_compiler_or_script_execution() {
        let artifacts = serde_json::json!({"reason":"compiler-artifact", "package_id":"path+file:///workspace#fixture@0.1.0", "manifest_path":"/workspace/Cargo.toml", "target":{"name":"fixture", "kind":["lib"], "crate_types":["rlib"], "src_path":"/workspace/src/lib.rs", "edition":"2024"}, "profile":{"opt_level":"0", "debuginfo":"line-tables-only", "test":false}, "features":["selected"], "filenames":["/output/target/libfixture.rmeta"], "executable":null, "fresh":true});
        let script = serde_json::json!({"reason":"build-script-executed", "package_id":"path+file:///workspace#fixture@0.1.0", "linked_libs":[], "linked_paths":[], "cfgs":["selected"], "env":[["TOKEN","private-value"],["TOKEN","replacement"]], "out_dir":"/output/target/build/out"});
        let bytes = format!(
            "a non-JSON native line\n{artifacts}\n{script}\n{{\"reason\":\"build-finished\",\"success\":true}}\n"
        );
        let output = CargoOutputObservation::parse(bytes.as_bytes(), true).unwrap();
        assert!(output.artifacts[0].fresh);
        assert_eq!(output.artifacts[0].profile["debuginfo"], "line-tables-only");
        assert_eq!(output.artifacts[0].target.remaining["edition"], "2024");
        let env = &output.build_scripts[0].environment_digests;
        assert_eq!(env.len(), 2);
        assert_eq!(env[0].0, env[1].0);
        assert_eq!(env[0].1, crate::integrity::framed_digest(b"private-value"));
        let serialized = serde_json::to_string(&output.build_scripts[0]).unwrap();
        assert!(!serialized.contains("private-value") && !serialized.contains("replacement"));
        assert!(CargoOutputObservation::parse(bytes.as_bytes(), false).is_err());
        assert!(CargoOutputObservation::parse(b"", true).is_err());
        assert!(CargoOutputObservation::parse(b"{bad}\n", false).is_err());
        assert!(
            CargoOutputObservation::parse(
                b"{\"reason\":\"build-finished\",\"success\":false}\n",
                false
            )
            .is_ok()
        );
    }
}
