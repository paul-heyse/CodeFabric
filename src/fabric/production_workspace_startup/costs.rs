//! Finite operational measurements for the latest source and semantic preparation.
//! These diagnostics do not participate in semantic identity or certify coverage.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use super::PublicationStage;

#[derive(Serialize)]
struct PhaseCost {
    phase: &'static str,
    elapsed_micros: u64,
    finished: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_generation: u64,
    stage: &'static str,
    source_files: usize,
    source_bytes: u64,
    relation_versions: usize,
    reused_relation_versions: usize,
    workspace_syntax_cache: Option<super::syntax_cache::SyntaxCacheObservation>,
    workspace_pyrefly_cache: Option<super::pyrefly_cache::PyreflyCacheObservation>,
    workspace_rust_toolchain_cache:
        Option<super::rustc::toolchain_cache::ToolchainCacheObservation>,
    native_cpu: Option<crate::resource_budget::native_cpu::NativeCpuObservation>,
    finished: bool,
    phases: Vec<PhaseCost>,
}

pub(super) struct PreparationCosts {
    path: PathBuf,
    report: Report,
    active: Option<(&'static str, Instant)>,
}

impl PreparationCosts {
    pub(super) fn new(root: &Path, generation: u64, stage: PublicationStage) -> Self {
        let stage = match stage {
            PublicationStage::Source => "source",
            PublicationStage::Semantic => "semantic",
        };
        Self {
            path: root.join(format!("{stage}-preparation-costs.json")),
            report: Report {
                schema: "codefabric.preparation-costs.v1",
                source_generation: generation,
                stage,
                source_files: 0,
                source_bytes: 0,
                relation_versions: 0,
                reused_relation_versions: 0,
                workspace_syntax_cache: None,
                workspace_pyrefly_cache: None,
                workspace_rust_toolchain_cache: None,
                native_cpu: None,
                finished: false,
                phases: Vec::with_capacity(12),
            },
            active: Some(("capture", Instant::now())),
        }
    }

    pub(super) fn inputs(&mut self, files: usize, bytes: u64) {
        self.report.source_files = files;
        self.report.source_bytes = bytes;
    }

    pub(super) fn syntax_cache(
        &mut self,
        observation: super::syntax_cache::SyntaxCacheObservation,
    ) {
        self.report.workspace_syntax_cache = Some(observation);
    }

    pub(super) fn pyrefly_cache(
        &mut self,
        observation: super::pyrefly_cache::PyreflyCacheObservation,
    ) {
        self.report.workspace_pyrefly_cache = Some(observation);
    }

    pub(super) fn native_cpu(
        &mut self,
        observation: crate::resource_budget::native_cpu::NativeCpuObservation,
    ) {
        self.report.native_cpu = Some(observation);
    }

    pub(super) fn start(&mut self, phase: &'static str) {
        self.end(true);
        self.active = Some((phase, Instant::now()));
    }

    pub(super) fn rust_toolchain_cache(
        &mut self,
        observation: super::rustc::toolchain_cache::ToolchainCacheObservation,
    ) {
        self.report.workspace_rust_toolchain_cache = Some(observation);
    }

    fn end(&mut self, finished: bool) {
        if let Some((phase, start)) = self.active.take() {
            self.report.phases.push(PhaseCost {
                phase,
                elapsed_micros: u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX),
                finished,
            });
        }
    }

    pub(super) fn finish(&mut self, relation_versions: usize, reused_relation_versions: usize) {
        self.end(true);
        self.report.relation_versions = relation_versions;
        self.report.reused_relation_versions = reused_relation_versions;
        self.report.finished = true;
    }

    fn persist(&self) -> std::io::Result<()> {
        // The joined workspace publication owner serializes these writes. Two replaceable
        // reports and staging files bound retention independently of source generations.
        let staged = self.path.with_extension("json.tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&staged)?;
        serde_json::to_writer(&mut file, &self.report)?;
        file.write_all(b"\n")?;
        drop(file);
        std::fs::rename(staged, &self.path)
    }
}

impl Drop for PreparationCosts {
    fn drop(&mut self) {
        self.end(false);
        if let Err(error) = self.persist() {
            tracing::warn!(%error, "preparation cost diagnostics could not be written");
        }
    }
}
