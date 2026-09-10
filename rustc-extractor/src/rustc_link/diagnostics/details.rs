//! Native diagnostic structure and source locations; no rendered filename is an identity.

use std::collections::BTreeMap;

use rustc_errors::formatting::{format_diag_message, format_diag_messages};
use rustc_errors::{Applicability, DiagInner, SuggestionStyle, Suggestions};
use rustc_span::source_map::SourceMap;
use rustc_span::{FileName, Span};

use super::super::{OwnedCell, OwnedRow, RustcRelation};
use super::{MAX_CAPTURE_BYTES, MAX_DIAGNOSTICS};

pub(super) const RELATIONS: [RustcRelation; 4] = [
    RustcRelation::DiagnosticChild,
    RustcRelation::DiagnosticSpan,
    RustcRelation::DiagnosticSuggestion,
    RustcRelation::DiagnosticEdit,
];

#[derive(Default)]
pub(super) struct Details {
    pub rows: BTreeMap<RustcRelation, Vec<OwnedRow>>,
    pub incomplete: bool,
    pub bytes: usize,
    pub count: usize,
}

pub(super) fn row_bytes(row: &OwnedRow) -> usize {
    row.0.values().fold(0_usize, |total, value| {
        total.saturating_add(128).saturating_add(match value {
            OwnedCell::Utf8(value) => value.len(),
            OwnedCell::Binary(value) => value.len(),
            _ => 32,
        })
    })
}

impl Details {
    fn push(&mut self, relation: RustcRelation, row: OwnedRow) -> bool {
        let bytes = row_bytes(&row);
        if self.count == MAX_DIAGNOSTICS || bytes > MAX_CAPTURE_BYTES.saturating_sub(self.bytes) {
            self.incomplete = true;
            return false;
        }
        self.bytes += bytes;
        self.count += 1;
        self.rows.entry(relation).or_default().push(row);
        true
    }

    pub fn capture(diag: &DiagInner, source_map: Option<&SourceMap>, ordinal: usize) -> Self {
        let mut result = Self::default();
        for (child_index, child) in diag.children.iter().enumerate() {
            if !result.push(
                RustcRelation::DiagnosticChild,
                OwnedRow::default()
                    .u64("diagnostic_ordinal", ordinal)
                    .u64("child_ordinal", child_index)
                    .utf8("severity", child.level.to_str())
                    .utf8("message", format_diag_messages(&child.messages, &diag.args)),
            ) {
                break;
            }
        }
        for (child_index, span) in std::iter::once((None, &diag.span)).chain(
            diag.children
                .iter()
                .enumerate()
                .map(|(index, child)| (Some(index), &child.span)),
        ) {
            if result.incomplete {
                break;
            }
            for (span_index, label) in span.span_labels().into_iter().enumerate() {
                let row = OwnedRow::default()
                    .u64("diagnostic_ordinal", ordinal)
                    .maybe_u64("child_ordinal", child_index)
                    .u64("span_ordinal", span_index)
                    .boolean("is_primary", label.is_primary)
                    .maybe_utf8(
                        "label",
                        label
                            .label
                            .as_ref()
                            .map(|label| format_diag_message(label, &diag.args)),
                    );
                if !result.push(
                    RustcRelation::DiagnosticSpan,
                    location(row, label.span, source_map),
                ) {
                    break;
                }
            }
        }
        result.suggestions(diag, source_map, ordinal);
        result
    }

    fn suggestions(&mut self, diag: &DiagInner, source_map: Option<&SourceMap>, ordinal: usize) {
        let suggestions = match &diag.suggestions {
            Suggestions::Enabled(values) => values.as_slice(),
            Suggestions::Sealed(values) => values.as_ref(),
            Suggestions::Disabled => &[],
        };
        for (suggestion_index, suggestion) in suggestions.iter().enumerate() {
            if self.incomplete {
                break;
            }
            let style = match suggestion.style {
                SuggestionStyle::HideCodeInline => "HideCodeInline",
                SuggestionStyle::HideCodeAlways => "HideCodeAlways",
                SuggestionStyle::CompletelyHidden => "CompletelyHidden",
                SuggestionStyle::ShowCode => "ShowCode",
                SuggestionStyle::ShowAlways => "ShowAlways",
            };
            let applicability = match suggestion.applicability {
                Applicability::MachineApplicable => "MachineApplicable",
                Applicability::MaybeIncorrect => "MaybeIncorrect",
                Applicability::HasPlaceholders => "HasPlaceholders",
                Applicability::Unspecified => "Unspecified",
            };
            let base = OwnedRow::default()
                .u64("diagnostic_ordinal", ordinal)
                .u64("suggestion_ordinal", suggestion_index)
                .utf8("message", format_diag_message(&suggestion.msg, &diag.args))
                .utf8("style", style)
                .utf8("applicability", applicability)
                .u64("alternative_count", suggestion.substitutions.len());
            if suggestion.substitutions.is_empty() {
                self.push(
                    RustcRelation::DiagnosticSuggestion,
                    base.u64("part_count", 0),
                );
            } else {
                for (alternative_index, alternative) in suggestion.substitutions.iter().enumerate()
                {
                    if !self.push(
                        RustcRelation::DiagnosticSuggestion,
                        base.clone()
                            .u64("alternative_ordinal", alternative_index)
                            .u64("part_count", alternative.parts.len()),
                    ) {
                        break;
                    }
                    for (part_index, part) in alternative.parts.iter().enumerate() {
                        let row = OwnedRow::default()
                            .u64("diagnostic_ordinal", ordinal)
                            .u64("suggestion_ordinal", suggestion_index)
                            .u64("alternative_ordinal", alternative_index)
                            .u64("part_ordinal", part_index)
                            .utf8("replacement_text", &part.snippet);
                        if !self.push(
                            RustcRelation::DiagnosticEdit,
                            location(row, part.span, source_map),
                        ) {
                            break;
                        }
                    }
                    if self.incomplete {
                        break;
                    }
                }
            }
        }
    }
}

fn location(mut row: OwnedRow, span: Span, source_map: Option<&SourceMap>) -> OwnedRow {
    row = row.utf8(
        "expansion_kind",
        if span.from_expansion() {
            "macro-expansion"
        } else {
            "source-authored"
        },
    );
    if span.is_dummy() {
        return row.utf8("location_state", "dummy-span");
    }
    let Some(source_map) = source_map else {
        return row.utf8("location_state", "source-map-unavailable");
    };
    let start = source_map.lookup_byte_offset(span.lo());
    let end = source_map.lookup_byte_offset(span.hi());
    row = row.utf8(
        "span_file",
        start.sf.name.prefer_remapped_unconditionally().to_string(),
    );
    if start.sf.start_pos != end.sf.start_pos {
        return row.utf8("location_state", "cross-file-span");
    }
    row = row
        .u64(
            "span_start_byte",
            start.sf.original_relative_byte_pos(span.lo()).0,
        )
        .u64(
            "span_end_byte",
            start.sf.original_relative_byte_pos(span.hi()).0,
        );
    let FileName::Real(name) = &start.sf.name else {
        return row.utf8("location_state", "non-file-span");
    };
    let Some(path) = name.local_path() else {
        return row.utf8("location_state", "local-path-unavailable");
    };
    row.0.insert(
        "span_file_bytes",
        OwnedCell::Binary(path.as_os_str().as_encoded_bytes().to_vec()),
    );
    row.utf8("location_state", "native-file-unbound")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_errors::{CodeSuggestion, Level, Subdiag, Substitution, SubstitutionPart};
    use rustc_span::DUMMY_SP;

    #[test]
    fn child_notes_and_alternative_multipart_edits_keep_their_native_structure() {
        let mut diag = DiagInner::new(Level::Error, "parent");
        diag.children.push(Subdiag {
            level: Level::Note,
            messages: DiagInner::new(Level::Note, "child").messages,
            span: DUMMY_SP.into(),
        });
        diag.suggestions = Suggestions::Enabled(vec![CodeSuggestion {
            msg: "try this".into(),
            style: SuggestionStyle::ShowCode,
            applicability: Applicability::MaybeIncorrect,
            substitutions: vec![
                Substitution {
                    parts: vec![
                        SubstitutionPart {
                            span: DUMMY_SP,
                            snippet: "a".into(),
                        },
                        SubstitutionPart {
                            span: DUMMY_SP,
                            snippet: "b".into(),
                        },
                    ],
                },
                Substitution { parts: vec![] },
            ],
        }]);
        let rows = Details::capture(&diag, None, 9);
        assert!(!rows.incomplete);
        assert_eq!(
            rows.rows[&RustcRelation::DiagnosticChild][0].0["diagnostic_ordinal"],
            OwnedCell::UInt64(9)
        );
        let alternatives = &rows.rows[&RustcRelation::DiagnosticSuggestion];
        assert_eq!(alternatives.len(), 2);
        assert_eq!(alternatives[1].0["part_count"], OwnedCell::UInt64(0));
        let edits = &rows.rows[&RustcRelation::DiagnosticEdit];
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[1].0["part_ordinal"], OwnedCell::UInt64(1));
        assert_eq!(edits[1].0["replacement_text"], OwnedCell::Utf8("b".into()));
        assert_eq!(
            edits[1].0["location_state"],
            OwnedCell::Utf8("dummy-span".into())
        );
        assert!(!edits[1].0.contains_key("location_file_id"));
    }
}
