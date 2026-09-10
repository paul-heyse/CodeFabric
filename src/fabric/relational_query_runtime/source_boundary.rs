//! Authorization-owned paths narrow the already captured inventory; they never access disk.

use crate::relational_program::{FieldId, ScalarExpression as E, ScalarOperator as O};
use datafusion::common::ScalarValue;
use datafusion::logical_expr::{Expr, lit};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct SourceBoundaries(Vec<String>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathBoundary {
    kind: PathKind,
    root: String,
}

#[derive(Deserialize)]
enum PathKind {
    #[serde(rename = "path")]
    Path,
}

impl SourceBoundaries {
    pub(crate) fn authorize(operands: &[String]) -> Result<Self, String> {
        if operands.len() > 256 {
            return Err("too many source boundaries".into());
        }
        let mut roots = operands
            .iter()
            .map(|operand| {
                let PathBoundary {
                    kind: PathKind::Path,
                    root,
                } = serde_json::from_str(operand).map_err(|_| "unsupported source boundary")?;
                Ok(root)
            })
            .collect::<Result<Vec<_>, String>>()?;
        roots.sort();
        roots.dedup();
        let value = Self(roots);
        if !value.valid() {
            return Err(
                "source boundaries require relative captured paths without traversal".into(),
            );
        }
        Ok(value)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn resolved(&self) -> Vec<serde_json::Value> {
        self.0
            .iter()
            .map(|root| serde_json::json!({"kind":"path", "root":root}))
            .collect()
    }

    pub(crate) fn valid(&self) -> bool {
        self.0.len() <= 256
            && self.0.windows(2).all(|pair| pair[0] < pair[1])
            && self.0.iter().all(|root| {
                root == "."
                    || (!root.is_empty()
                        && root.len() <= 4096
                        && !root.contains('\0')
                        && root.split('/').all(|part| !matches!(part, "" | "." | "..")))
            })
    }

    pub(crate) fn selects(&self, path: &[u8]) -> bool {
        self.is_empty()
            || self.0.iter().any(|root| {
                root == "."
                    || path == root.as_bytes()
                    || path
                        .strip_prefix(root.as_bytes())
                        .is_some_and(|rest| rest.starts_with(b"/"))
            })
    }

    // A slash-delimited subtree is one binary range [root + '/', root + '0'). This
    // preserves non-UTF8 descendants and makes %, _, and backslash ordinary path bytes.
    fn ranges(&self) -> impl Iterator<Item = (Vec<u8>, Vec<u8>, Vec<u8>)> + '_ {
        self.0.iter().map(|root| {
            let exact = root.as_bytes().to_vec();
            let mut lower = exact.clone();
            lower.push(b'/');
            let mut upper = exact.clone();
            upper.push(b'0');
            (exact, lower, upper)
        })
    }

    pub(crate) fn predicate(&self, field: &FieldId) -> E {
        if self.is_empty() || self.0.iter().any(|root| root == ".") {
            return E::Literal(ScalarValue::Boolean(Some(true)));
        }
        let terms = self
            .ranges()
            .map(|(exact, lower, upper)| {
                let compare = |operator, value| E::Call {
                    operator,
                    arguments: vec![
                        E::Field(field.clone()),
                        E::Literal(ScalarValue::Binary(Some(value))),
                    ],
                };
                E::Call {
                    operator: O::Or,
                    arguments: vec![
                        compare(O::Equal, exact),
                        E::Call {
                            operator: O::And,
                            arguments: vec![
                                compare(O::GreaterThanOrEqual, lower),
                                compare(O::LessThan, upper),
                            ],
                        },
                    ],
                }
            })
            .collect();
        balanced_union(terms, |left, right| E::Call {
            operator: O::Or,
            arguments: vec![left, right],
        })
    }

    pub(crate) fn native_predicate(&self, field: &Expr) -> Expr {
        if self.is_empty() || self.0.iter().any(|root| root == ".") {
            return lit(true);
        }
        let terms = self
            .ranges()
            .map(|(exact, lower, upper)| {
                let bytes = |value| lit(ScalarValue::Binary(Some(value)));
                field.clone().eq(bytes(exact)).or(field
                    .clone()
                    .gt_eq(bytes(lower))
                    .and(field.clone().lt(bytes(upper))))
            })
            .collect();
        balanced_union(terms, Expr::or)
    }
}

// Keep both application and native predicate trees logarithmic in the bounded operand count.
fn balanced_union<T>(mut terms: Vec<T>, combine: impl Fn(T, T) -> T) -> T {
    while terms.len() > 1 {
        let mut next = Vec::with_capacity(terms.len().div_ceil(2));
        let mut terms_iter = terms.into_iter();
        while let Some(left) = terms_iter.next() {
            next.push(match terms_iter.next() {
                Some(right) => combine(left, right),
                None => left,
            });
        }
        terms = next;
    }
    terms.pop().expect("nonempty boundary selection")
}

impl super::SelectedQueryOutput {
    /// File-anchored outputs are narrowed before limit/probe; unanchored schemas fail explicitly.
    pub(crate) fn with_source_boundaries(
        mut self,
        boundaries: &SourceBoundaries,
    ) -> Result<Self, String> {
        use crate::relational_program::{JoinKind, RelationId, RelationalExpression as R};
        if boundaries.is_empty() {
            return Ok(self);
        }
        let binding = self
            .program_result_binding
            .as_ref()
            .ok_or("source boundary output schema unavailable")?;
        let file_index = binding
            .schema()
            .fields()
            .iter()
            .position(|field| matches!(field.name().as_str(), "file_id" | "source-file-id"))
            .ok_or("source boundary requires a file-anchored result family")?;
        let field = |name| {
            FieldId::new(format!("source.code_file.{name}")).map_err(|error| error.to_string())
        };
        let selected = R::Filter {
            input: Box::new(R::Input(
                RelationId::new("source.code_file").map_err(|error| error.to_string())?,
            )),
            predicate: boundaries.predicate(&field("relative_path")?),
        };
        let predicate = E::Call {
            operator: O::Equal,
            arguments: vec![
                E::Field(binding.field_ids()[file_index].clone()),
                E::Field(field("file_id")?),
            ],
        };
        let narrow = |input| R::Join {
            left: Box::new(input),
            right: Box::new(selected),
            kind: JoinKind::LeftSemi,
            predicates: vec![predicate],
        };
        self.program.root = match self.program.root {
            R::Limit { input, skip, fetch } => R::Limit {
                input: Box::new(narrow(*input)),
                skip,
                fetch,
            },
            root => narrow(root),
        };
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{BinaryArray, RecordBatch};
    use datafusion::logical_expr::col;
    use datafusion::prelude::SessionContext;
    use std::sync::Arc;

    #[tokio::test]
    async fn path_boundaries_use_native_binary_ranges_and_literal_components() {
        let boundaries = SourceBoundaries::authorize(&[
            r#"{"kind":"path","root":"src_%"}"#.into(),
            r#"{"kind":"path","root":"one.py"}"#.into(),
        ])
        .unwrap();
        let paths: Vec<&[u8]> = vec![
            b"src_%",
            b"src_%/a.py",
            b"src_%/\xff.py",
            b"one.py",
            b"src_%other/b.py",
            b"src_XX/a.py",
            b"src_%0",
            b"oneXpy",
        ];
        let expected = &paths[..4];
        assert_eq!(
            paths
                .iter()
                .copied()
                .filter(|path| boundaries.selects(path))
                .collect::<Vec<_>>(),
            expected
        );
        let batch =
            RecordBatch::try_from_iter([("path", Arc::new(BinaryArray::from(paths.clone())) as _)])
                .unwrap();
        let batches = SessionContext::new()
            .read_batch(batch)
            .unwrap()
            .filter(boundaries.native_predicate(&col("path")))
            .unwrap()
            .collect()
            .await
            .unwrap();
        let actual = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .unwrap()
                    .iter()
                    .map(|value| value.unwrap().to_vec())
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        for invalid in [
            "",
            "/etc",
            "../src",
            "src/../other",
            "src//lib",
            "src/",
            "src/./lib",
            "src\0x",
        ] {
            assert!(
                SourceBoundaries::authorize(&[
                    serde_json::json!({"kind":"path","root":invalid}).to_string()
                ])
                .is_err()
            );
        }
        for invalid in [
            r#"{"kind":"glob","root":"**"}"#,
            r#"{"kind":"path","root":"src","fetch":true}"#,
        ] {
            assert!(SourceBoundaries::authorize(&[invalid.into()]).is_err());
        }
        let all = SourceBoundaries::authorize(&[r#"{"kind":"path","root":"."}"#.into()]).unwrap();
        assert!(all.selects(b"any/\xff"));
        assert!(
            !serde_json::from_str::<SourceBoundaries>(r#"["../private"]"#)
                .unwrap()
                .valid()
        );
    }
}
