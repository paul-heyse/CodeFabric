//! Original-source coordinates are typed operands, separate from controlled semantic phrases.

use serde::{Deserialize, Serialize};

/// A point or half-open interval in one file of the selected captured source inventory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLocation {
    /// Canonical workspace-relative path; this operand never opens a live pathname.
    pub source_file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One-based starting line, mutually exclusive with `start_byte`.
    pub start_line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Zero-based original-byte column; omitted columns select column zero.
    pub start_column: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional exclusive ending line of a line/column interval.
    pub end_line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Zero-based original-byte column at the exclusive end.
    pub end_column: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Zero-based original byte offset, mutually exclusive with `start_line`.
    pub start_byte: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional exclusive end; an omitted or equal end describes a point.
    pub end_byte: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional controlled entity meaning, with literal raw syntax kinds preserved.
    pub semantic_location: Option<String>,
}

impl SourceLocation {
    /// Coordinates address captured original bytes: lines are one-based and columns zero-based.
    ///
    /// # Errors
    /// Rejects noncanonical paths, ambiguous coordinate bases and reversed or oversized operands.
    pub fn validate(&self) -> Result<(), String> {
        if self.source_file.is_empty()
            || self.source_file.len() > 4096
            || self.source_file.contains('\0')
            || self
                .source_file
                .split('/')
                .any(|part| matches!(part, "" | "." | ".."))
        {
            return Err("source location requires a canonical captured relative file path".into());
        }
        match (self.start_line, self.start_byte) {
            (Some(line), None) if line > 0 => {
                if self.end_byte.is_some()
                    || self.end_column.is_some() && self.end_line.is_none()
                    || self.end_line.is_some_and(|end| {
                        (end, self.end_column.unwrap_or(0)) < (line, self.start_column.unwrap_or(0))
                    })
                {
                    return Err("source location has inconsistent line coordinates".into());
                }
            }
            (None, Some(start)) => {
                if self.start_column.is_some()
                    || self.end_line.is_some()
                    || self.end_column.is_some()
                    || self.end_byte.is_some_and(|end| end < start)
                {
                    return Err("source location has inconsistent byte coordinates".into());
                }
            }
            _ => {
                return Err(
                    "source location requires either a positive start line or a byte offset".into(),
                );
            }
        }
        if self
            .semantic_location
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 512)
        {
            return Err("source location semantic meaning is empty or exceeds its bound".into());
        }
        Ok(())
    }

    pub(crate) fn meaning(&self) -> Result<LocationMeaning<'_>, String> {
        let Some(mut phrase) = self.semantic_location.as_deref() else {
            return Ok(LocationMeaning::default());
        };
        phrase = phrase.strip_prefix("the ").unwrap_or(phrase);
        let language = if let Some(rest) = phrase.strip_prefix("Python ") {
            phrase = rest;
            Some("python")
        } else if let Some(rest) = phrase.strip_prefix("Rust ") {
            phrase = rest;
            Some("rust")
        } else {
            None
        };
        let begins_on_line = phrase.ends_with(" beginning on this line");
        if begins_on_line {
            phrase = phrase
                .strip_suffix(" beginning on this line")
                .expect("checked suffix");
            if self.start_line.is_none() {
                return Err("line-based semantic locations require a start line".into());
            }
        }
        let mut raw_kind = None;
        let kind = match phrase {
            "function" | "function declaration" => "function",
            "call" | "call expression" | "call occurrence" => "call",
            "reference" | "reference occurrence" => "reference",
            "import" | "import occurrence" => "import-occurrence",
            "module" => "module",
            "syntax node" => "syntax-node",
            value if value.starts_with("syntax node `") && value.ends_with('`') => {
                let value = &value[13..value.len() - 1];
                if value.is_empty() || value.contains('`') {
                    return Err("invalid literal syntax kind in source location".into());
                }
                raw_kind = Some(value);
                "syntax-node"
            }
            _ => return Err("source location semantic meaning is unavailable".into()),
        };
        Ok(LocationMeaning {
            kind: Some(kind),
            language,
            raw_kind,
            begins_on_line,
        })
    }
}

#[derive(Default)]
pub(crate) struct LocationMeaning<'a> {
    pub kind: Option<&'static str>,
    pub language: Option<&'static str>,
    pub raw_kind: Option<&'a str>,
    pub begins_on_line: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn location_coordinates_are_typed_bounded_and_unambiguous() {
        for value in [
            json!({"source_file":"src/main.rs","start_line":1}),
            json!({"source_file":"café.py","start_line":2,"start_column":0,"end_line":2,"end_column":4}),
            json!({"source_file":"src/literal%_.rs","start_byte":0,"end_byte":0}),
        ] {
            let location: SourceLocation = serde_json::from_value(value).unwrap();
            location.validate().unwrap();
            assert_eq!(
                location,
                serde_json::from_slice::<SourceLocation>(&serde_json::to_vec(&location).unwrap())
                    .unwrap()
            );
        }
        for value in [
            json!({"source_file":"../src/a.rs","start_line":1}),
            json!({"source_file":"/src/a.rs","start_line":1}),
            json!({"source_file":"src//a.rs","start_line":1}),
            json!({"source_file":"a.rs","start_line":0}),
            json!({"source_file":"a.rs","start_line":1,"start_byte":0}),
            json!({"source_file":"a.rs","start_column":1}),
            json!({"source_file":"a.rs","start_line":2,"end_line":1}),
            json!({"source_file":"a.rs","start_byte":2,"end_byte":1}),
        ] {
            assert!(
                serde_json::from_value::<SourceLocation>(value)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        assert!(
            serde_json::from_value::<SourceLocation>(
                json!({"source_file":"a.rs","start_line":1,"surprise":true})
            )
            .is_err()
        );
    }

    #[test]
    fn semantic_location_keeps_literal_raw_kinds_separate() {
        let location: SourceLocation = serde_json::from_value(json!({"source_file":"a.py", "start_line":1, "semantic_location":"Python syntax node `.`"})).unwrap();
        let meaning = location.meaning().unwrap();
        assert_eq!(meaning.kind, Some("syntax-node"));
        assert_eq!(meaning.raw_kind, Some("."));
        assert_eq!(meaning.language, Some("python"));
        let mut location = location;
        location.semantic_location = Some("the call expression beginning on this line".into());
        assert!(location.meaning().unwrap().begins_on_line);
        location.semantic_location = Some("safe to refactor".into());
        assert!(location.meaning().is_err());
    }
}
