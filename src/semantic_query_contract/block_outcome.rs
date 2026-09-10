//! Independent block execution outcomes, separate from provider processing completeness.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueryBlockExecutionState {
    Complete,
    Failed,
    NotExecutedDependency,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryBlockIssue {
    pub code: String,
    pub subject_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryBlockOutcome {
    pub query_id: String,
    pub execution_state: QueryBlockExecutionState,
    pub errors: Vec<QueryBlockIssue>,
}

impl QueryBlockOutcome {
    pub(crate) fn valid(&self) -> bool {
        let text = |value: &str, maximum| {
            !value.is_empty() && value.len() <= maximum && !value.contains('\0')
        };
        text(&self.query_id, 128)
            && self.errors.len() <= 128
            && self.errors.iter().all(|error| {
                text(&error.code, 128)
                    && text(&error.subject_id, 512)
                    && error.related_id.as_deref().is_none_or(|id| text(id, 512))
            })
            && match self.execution_state {
                QueryBlockExecutionState::Complete => self.errors.is_empty(),
                QueryBlockExecutionState::Failed => !self.errors.is_empty(),
                QueryBlockExecutionState::NotExecutedDependency => {
                    !self.errors.is_empty()
                        && self.errors.iter().all(|error| {
                            error.code == "NOT_EXECUTED_DEPENDENCY" && error.related_id.is_some()
                        })
                }
            }
    }
}
