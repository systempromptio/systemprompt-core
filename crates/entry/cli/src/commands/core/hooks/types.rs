use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HookListOutput {
    pub hooks: Vec<HookEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HookEntry {
    pub plugin_id: String,
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
    pub plugin_id: String,
    pub valid: bool,
    pub errors: Vec<String>,
}
