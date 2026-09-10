//! The pinned native JSON emitter translates compiler messages. Only selected typed scalar fields
//! leave this seam; its display filenames are never used as application source identities.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use rustc_driver::{Callbacks, Compilation};
use rustc_errors::emitter::{ColorConfig, Emitter, HumanReadableErrorType, TimingEvent};
use rustc_errors::timings::TimingRecord;
use rustc_errors::{DiagInner, Suggestions};
use rustc_span::source_map::SourceMap;

mod details;
use rustc_errors::json::JsonEmitter;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;
use serde::Deserialize;

use super::{OwnedRow, OwnedRustcExtraction, OwnedRustcOwner, OwnedRustcRelation, RustcRelation};

const MAX_CAPTURE_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIAGNOSTICS: usize = 20_000;

#[derive(Default)]
struct Capture {
    rows: Vec<OwnedRow>,
    details: details::Details,
    detail_count: usize,
    captured_bytes: usize,
    initialized: bool,
    incomplete: bool,
}

#[derive(Deserialize)]
struct Diagnostic {
    #[serde(rename = "$message_type")]
    message_type: String,
    message: Option<String>,
    level: Option<String>,
    code: Option<DiagnosticCode>,
}

#[derive(Deserialize)]
struct DiagnosticCode {
    code: String,
}

impl Capture {
    fn record(&mut self, frame: &[u8]) {
        let Ok(diagnostic) = serde_json::from_slice::<Diagnostic>(frame) else {
            self.incomplete = true;
            return;
        };
        if diagnostic.message_type != "diagnostic" {
            return;
        }
        let (Some(message), Some(level)) = (diagnostic.message, diagnostic.level) else {
            self.incomplete = true;
            return;
        };
        if self.rows.len() == MAX_DIAGNOSTICS
            || frame.len() > MAX_CAPTURE_BYTES.saturating_sub(self.captured_bytes)
        {
            self.incomplete = true;
            return;
        }
        self.captured_bytes += frame.len();
        self.rows.push(
            OwnedRow::default()
                .u64("diagnostic_ordinal", self.rows.len())
                .utf8("severity", level)
                .utf8(
                    "reason_code",
                    diagnostic.code.map_or_else(String::new, |code| code.code),
                )
                .utf8("message", message)
                .boolean("structured_compiler_diagnostic", true)
                .utf8("suggestions_state", "unavailable"),
        );
    }

    fn append_to(&mut self, owner: &mut OwnedRustcOwner) {
        let complete = self.initialized && !self.incomplete;
        let mut relations = std::mem::take(&mut self.details.rows);
        relations.insert(RustcRelation::Diagnostic, std::mem::take(&mut self.rows));
        for relation in std::iter::once(RustcRelation::Diagnostic).chain(details::RELATIONS) {
            let rows = relations.remove(&relation).unwrap_or_default();
            let count = rows.len();
            owner.relations.push(OwnedRustcRelation { relation, rows });
            append_row(
                owner,
                RustcRelation::Coverage,
                OwnedRow::default()
                    .utf8("fact_family", relation.relation_id())
                    .utf8(
                        "authority_surface",
                        "rustc_errors::DiagInner / JsonEmitter / SourceMap",
                    )
                    .u64("requested_units", 1)
                    .u64("completed_units", usize::from(complete))
                    .u64("emitted_rows", count)
                    .utf8(
                        "completeness",
                        if complete {
                            "complete"
                        } else {
                            "partial-characterized"
                        },
                    )
                    .u64("remainder_count", usize::from(!complete))
                    .boolean("unknown_semantics", !complete),
            );
            if !complete {
                append_row(owner, RustcRelation::Remainder, OwnedRow::default()
                    .utf8("fact_family", relation.relation_id())
                    .utf8("reason_code", if self.initialized { "STRUCTURED_DIAGNOSTIC_CAPTURE_LIMIT_OR_INVALID_FRAME" } else { "STRUCTURED_DIAGNOSTIC_SINK_NOT_INITIALIZED" })
                    .utf8("authority_surface", "rustc_errors::DiagInner / JsonEmitter")
                    .boolean("bounded", true)
                    .utf8("detail", "diagnostic capture is incomplete; missing messages, locations and edits are unknown"));
            }
        }
    }
}

fn append_row(owner: &mut OwnedRustcOwner, relation: RustcRelation, row: OwnedRow) {
    if let Some(existing) = owner
        .relations
        .iter_mut()
        .find(|item| item.relation == relation)
    {
        existing.rows.push(row);
    } else {
        owner.relations.push(OwnedRustcRelation {
            relation,
            rows: vec![row],
        });
    }
}

/// Preserve Cargo's diagnostic stream while bounding the additional application-owned copy. The
/// native emitter can write a frame in arbitrarily small fragments, including the final newline.
struct DiagnosticWriter {
    capture: Arc<Mutex<Capture>>,
    frame: Vec<u8>,
    oversized: bool,
    output: Box<dyn Write + Send>,
}

impl Write for DiagnosticWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.write_all(bytes)?;
        for fragment in bytes.split_inclusive(|byte| *byte == b'\n') {
            if fragment.len() > MAX_CAPTURE_BYTES.saturating_sub(self.frame.len()) {
                self.oversized = true;
                self.frame.clear();
            }
            if !self.oversized {
                self.frame.extend_from_slice(fragment);
            }
            if fragment.last() == Some(&b'\n') {
                let mut capture = self
                    .capture
                    .lock()
                    .expect("diagnostic capture is not poisoned");
                if self.oversized {
                    capture.incomplete = true;
                } else {
                    capture.record(&self.frame);
                }
                self.frame.clear();
                self.oversized = false;
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

impl Drop for DiagnosticWriter {
    fn drop(&mut self) {
        if self.oversized || !self.frame.is_empty() {
            self.capture
                .lock()
                .expect("diagnostic capture is not poisoned")
                .incomplete = true;
        }
    }
}

/// Capture typed details before the native emitter consumes the diagnostic. The native JSON
/// primary remains authoritative for lint names, which are private in this compiler revision.
/// Both the pending detail bundle and retained capture are bounded independently.
struct StructuredEmitter {
    inner: JsonEmitter,
    capture: Arc<Mutex<Capture>>,
}

impl Emitter for StructuredEmitter {
    fn emit_diagnostic(&mut self, diag: DiagInner) {
        let ordinal = self
            .capture
            .lock()
            .expect("diagnostic capture is not poisoned")
            .rows
            .len();
        let details = details::Details::capture(&diag, self.inner.source_map(), ordinal);
        let suggestions_state = match &diag.suggestions {
            Suggestions::Enabled(_) => "enabled",
            Suggestions::Sealed(_) => "sealed",
            Suggestions::Disabled => "disabled",
        };
        self.inner.emit_diagnostic(diag);
        let mut capture = self
            .capture
            .lock()
            .expect("diagnostic capture is not poisoned");
        if capture.rows.len() != ordinal + 1 {
            capture.incomplete = true;
            return;
        }
        capture.rows[ordinal].0.insert(
            "suggestions_state",
            super::OwnedCell::Utf8(suggestions_state.into()),
        );
        capture.incomplete |= details.incomplete;
        if details.count > MAX_DIAGNOSTICS.saturating_sub(capture.detail_count)
            || details.bytes > MAX_CAPTURE_BYTES.saturating_sub(capture.captured_bytes)
        {
            capture.incomplete = true;
            return;
        }
        capture.captured_bytes += details.bytes;
        capture.detail_count += details.count;
        for (relation, rows) in details.rows {
            capture
                .details
                .rows
                .entry(relation)
                .or_default()
                .extend(rows);
        }
    }

    fn source_map(&self) -> Option<&SourceMap> {
        self.inner.source_map()
    }
    fn emit_artifact_notification(&mut self, path: &std::path::Path, kind: &str) {
        self.inner.emit_artifact_notification(path, kind);
    }
    fn emit_timing_section(&mut self, record: TimingRecord, event: TimingEvent) {
        self.inner.emit_timing_section(record, event);
    }
    fn emit_future_breakage_report(&mut self, diags: Vec<DiagInner>) {
        // The regular emission supplies these diagnostic observations. This report is Cargo's
        // separate compatibility envelope, not an additional set of compiler occurrences.
        self.inner.emit_future_breakage_report(diags);
    }
    fn emit_unused_externs(&mut self, level: rustc_lint_defs::Level, names: &[&str]) {
        self.inner.emit_unused_externs(level, names);
    }
    fn should_show_explain(&self) -> bool {
        self.inner.should_show_explain()
    }
    fn supports_color(&self) -> bool {
        self.inner.supports_color()
    }
}

struct ExtractorCallbacks {
    capture: Arc<Mutex<Capture>>,
    extraction: Option<OwnedRustcExtraction>,
}

impl Callbacks for ExtractorCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        let capture = Arc::clone(&self.capture);
        config.psess_created = Some(Box::new(move |session| {
            let emitter = JsonEmitter::new(
                Box::new(DiagnosticWriter {
                    capture: Arc::clone(&capture),
                    frame: Vec::new(),
                    oversized: false,
                    output: Box::new(io::stderr()),
                }),
                Some(session.clone_source_map()),
                false,
                HumanReadableErrorType {
                    short: false,
                    unicode: false,
                },
                ColorConfig::Never,
            );
            session.dcx().set_emitter(Box::new(StructuredEmitter {
                inner: emitter,
                capture: Arc::clone(&capture),
            }));
            capture
                .lock()
                .expect("diagnostic capture is not poisoned")
                .initialized = true;
        }));
    }

    fn after_analysis(&mut self, _compiler: &interface::Compiler, tcx: TyCtxt<'_>) -> Compilation {
        rustc_public::rustc_internal::run(tcx, || {
            self.extraction = super::extract_inside_callback(tcx).continue_value();
        })
        .expect("the pinned compiler bridge installs one callback context");
        Compilation::Continue
    }
}

pub(super) fn extract(arguments: &[String]) -> OwnedRustcExtraction {
    let capture = Arc::new(Mutex::new(Capture::default()));
    let mut callbacks = ExtractorCallbacks {
        capture: Arc::clone(&capture),
        extraction: None,
    };
    let compiler_completed = rustc_driver::catch_fatal_errors(|| {
        rustc_driver::run_compiler(arguments, &mut callbacks);
    })
    .is_ok();
    let mut extraction = callbacks
        .extraction
        .take()
        .unwrap_or_else(|| OwnedRustcExtraction {
            owners: vec![OwnedRustcOwner {
                qualified_name: "$compiler_diagnostics".to_owned(),
                owner_kind: "COMPILATION".to_owned(),
                compiler_key: None,
                relations: Vec::new(),
            }],
            compiler_succeeded: false,
        });
    // A driver invocation that never reached analysis does not establish compiler semantic success.
    extraction.compiler_succeeded = compiler_completed
        && extraction.owners[0]
            .relations
            .iter()
            .any(|item| item.relation == RustcRelation::Compilation);
    capture
        .lock()
        .expect("diagnostic capture is not poisoned")
        .append_to(&mut extraction.owners[0]);
    extraction
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustc_link::OwnedCell;

    #[test]
    fn native_diagnostic_frames_keep_typed_messages_across_fragments() {
        let capture = Arc::new(Mutex::new(Capture::default()));
        let mut writer = DiagnosticWriter {
            capture: Arc::clone(&capture),
            frame: Vec::new(),
            oversized: false,
            output: Box::new(io::sink()),
        };
        let frame = b"{\"$message_type\":\"diagnostic\",\"message\":\"expected u32\\nfound &str\",\"level\":\"error\",\"code\":{\"code\":\"E0308\"}}\n";
        for byte in frame {
            writer.write_all(&[*byte]).unwrap();
        }
        writer
            .write_all(b"{\"$message_type\":\"artifact\",\"artifact\":\"ignored\"}\n")
            .unwrap();
        let capture = capture.lock().unwrap();
        assert_eq!(capture.rows.len(), 1);
        assert!(!capture.incomplete);
        assert_eq!(
            capture.rows[0].0["reason_code"],
            OwnedCell::Utf8("E0308".into())
        );
        assert_eq!(
            capture.rows[0].0["message"],
            OwnedCell::Utf8("expected u32\nfound &str".into())
        );
    }

    #[test]
    fn oversized_and_unfinished_native_frames_leave_explicit_incomplete_capture() {
        let capture = Arc::new(Mutex::new(Capture::default()));
        {
            let mut writer = DiagnosticWriter {
                capture: Arc::clone(&capture),
                frame: Vec::new(),
                oversized: false,
                output: Box::new(io::sink()),
            };
            writer
                .write_all(&vec![b'a'; MAX_CAPTURE_BYTES + 1])
                .unwrap();
            assert_eq!(writer.frame, [] as [u8; 0]);
            writer.write_all(b"\n").unwrap();
            writer.write_all(b"unfinished").unwrap();
        }
        let capture = capture.lock().unwrap();
        assert!(capture.incomplete);
        assert_eq!(capture.rows, []);
    }
}
