//! Bounded cursor envelopes preserve complete traversal of retained
//! collections.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use serde::Serialize;
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}
