use crate::ai::ToolModelConfig;
use crate::auth::{JwtAudience, Permission};
use crate::mcp::capabilities::ToolVisibility;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolUiConfig {
    #[serde(default = "default_resource_uri_template")]
    pub resource_uri_template: String,
    #[serde(default = "default_visibility_enum")]
    pub visibility: Vec<ToolVisibility>,
}

fn default_resource_uri_template() -> String {
    "ui://systemprompt/{artifact_id}".to_string()
}

fn default_visibility_enum() -> Vec<ToolVisibility> {
    vec![ToolVisibility::Model, ToolVisibility::App]
}

impl ToolUiConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_template(mut self, template: impl Into<String>) -> Self {
        self.resource_uri_template = template.into();
        self
    }

    pub fn model_only(mut self) -> Self {
        self.visibility = vec![ToolVisibility::Model];
        self
    }

    pub fn model_and_app(mut self) -> Self {
        self.visibility = vec![ToolVisibility::Model, ToolVisibility::App];
        self
    }

    pub fn to_meta_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ui": {
                "resourceUri": self.resource_uri_template,
                "visibility": self.visibility
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolMetadata {
    #[serde(default)]
    pub terminal_on_success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<ToolModelConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<ToolUiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub deployments: HashMap<String, Deployment>,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub binary: String,
    pub package: Option<String>,
    pub port: u16,
    pub endpoint: String,
    pub enabled: bool,
    pub display_in_web: bool,
    #[serde(default)]
    pub dev_only: bool,
    #[serde(default)]
    pub schemas: Vec<SchemaDefinition>,
    pub oauth: OAuthRequirement,
    #[serde(default)]
    pub tools: HashMap<String, ToolMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<ToolModelConfig>,
    #[serde(default)]
    pub env_vars: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub file: String,
    pub table: String,
    pub required_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthRequirement {
    pub required: bool,
    pub scopes: Vec<Permission>,
    pub audience: JwtAudience,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub auto_build: bool,
    pub build_timeout: u64,
    pub health_check_timeout: u64,
    #[serde(default = "default_base_port")]
    pub base_port: u16,
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
}

const fn default_base_port() -> u16 {
    5000
}

fn default_working_dir() -> String {
    "/app".to_string()
}
