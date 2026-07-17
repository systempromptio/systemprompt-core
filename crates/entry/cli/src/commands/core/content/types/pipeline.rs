//! Publish-pipeline output types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportOutput {
    pub exported_count: i64,
    pub output_directory: String,
    pub files: Vec<String>,
}
