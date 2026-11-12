//! Output shapes for `plugins seams`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SeamStatus {
    Linked,
    Empty,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SeamRow {
    pub seam: String,
    pub owner: String,
    pub count: usize,
    pub entries: String,
    pub requires: String,
    pub status: SeamStatus,
}
