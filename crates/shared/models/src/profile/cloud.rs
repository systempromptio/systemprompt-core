//! Cloud configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudConfig {
    #[serde(default = "default_credentials_path")]
    pub credentials_path: String,

    #[serde(default = "default_tenants_path")]
    pub tenants_path: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    #[serde(default)]
    pub validation: CloudValidationMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CloudValidationMode {
    #[default]
    Strict,
    Warn,
    Skip,
}

fn default_credentials_path() -> String {
    "./credentials.json".to_string()
}

fn default_tenants_path() -> String {
    "./tenants.json".to_string()
}
