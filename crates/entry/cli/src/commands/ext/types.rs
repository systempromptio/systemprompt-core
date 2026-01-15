use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CliExtensionInfo {
    pub name: String,
    pub binary: String,
    pub description: String,
    pub commands: Vec<CliCommandInfo>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CliCommandInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtListOutput {
    pub extensions: Vec<CliExtensionInfo>,
    pub total: usize,
}
