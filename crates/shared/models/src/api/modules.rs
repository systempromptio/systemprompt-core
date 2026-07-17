//! Module descriptors exposed by the modules API.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub api_prefix: String,
    pub docs_url: String,
}
