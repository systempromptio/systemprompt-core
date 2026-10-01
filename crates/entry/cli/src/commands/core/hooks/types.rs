//! Hook-command entry types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::HookId;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HookListOutput {
    pub hooks: Vec<HookEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HookEntry {
    #[serde(rename = "plugin_id")]
    pub hook_id: HookId,
    pub event: String,
    pub matcher: String,
    pub hook_type: String,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HookValidateOutput {
    pub results: Vec<HookValidateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HookValidateEntry {
    #[serde(rename = "plugin_id")]
    pub hook_id: HookId,
    pub valid: bool,
    pub errors: Vec<String>,
}
