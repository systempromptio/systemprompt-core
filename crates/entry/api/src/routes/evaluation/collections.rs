//! Bounded cursor envelopes preserve complete traversal of retained
//! collections.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}
/// Common cursor bounds for collections whose identifiers are lexically
/// ordered.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    #[schemars(length(min = 1, max = 512))]
    pub after: Option<String>,
    #[schemars(range(min = 1, max = 100))]
    pub limit: Option<u32>,
}
impl Cursor {
    pub(super) fn limit(&self) -> Result<u32, super::optimization_error::OptimizationHttpError> {
        let limit = self.limit.unwrap_or(50);
        if !(1..=100).contains(&limit)
            || self
                .after
                .as_ref()
                .is_some_and(|id| id.is_empty() || id.len() > 512)
        {
            return Err(systemprompt_evaluation::EvaluationError::InvalidSpec(
                "Cursor requires a bounded identifier and limit 1–100".to_owned(),
            )
            .into());
        }
        Ok(limit)
    }
}
