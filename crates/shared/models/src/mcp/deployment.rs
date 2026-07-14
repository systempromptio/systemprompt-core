//! MCP server deployment configuration.
//!
//! [`DeploymentConfig`] is the top-level shape loaded from MCP service YAML:
//! a map of named [`Deployment`]s plus global [`Settings`]. Each deployment
//! declares its [`McpServerType`], OAuth requirement, schemas, and per-tool
//! [`ToolMetadata`]. Internal-server endpoints are validated relative by
//! [`Deployment::validate`].

use crate::ai::ToolModelConfig;
use crate::auth::{JwtAudience, Permission};
use crate::errors::ConfigValidationError;
use crate::mcp::capabilities::ToolVisibility;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use systemprompt_identifiers::ClientId;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpServerType {
    #[default]
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "external")]
    External,
}

impl McpServerType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::External => "external",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolUiConfig {
    #[serde(default = "default_resource_uri_template")]
    pub resource_uri_template: String,
    #[serde(default = "default_visibility_enum")]
    pub visibility: Vec<ToolVisibility>,
}

fn default_resource_uri_template() -> String {
    "ui://systemprompt/{artifact_id}".to_owned()
}

fn default_visibility_enum() -> Vec<ToolVisibility> {
    vec![ToolVisibility::Model, ToolVisibility::App]
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
    #[serde(default, alias = "type")]
    pub server_type: McpServerType,
    pub binary: String,
    pub package: Option<String>,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_auth: Option<ExternalAuth>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
}

/// Per-user bearer resolution for an `external` MCP server.
///
/// The MCP gateway exposes no token vault of its own; instead an extension
/// banks the calling user's third-party token and serves it from
/// `token_endpoint`. At tool-call time core `GET`s that accessor with the
/// user's systemprompt JWT and injects the returned bearer onto `header` (as
/// `{scheme} {token}`), replacing the systemprompt credential so nothing
/// internal reaches the third party.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAuth {
    pub token_endpoint: String,
    #[serde(default = "default_auth_header")]
    pub header: String,
    #[serde(default = "default_auth_scheme")]
    pub scheme: String,
}

fn default_auth_header() -> String {
    "Authorization".to_owned()
}

fn default_auth_scheme() -> String {
    "Bearer".to_owned()
}

impl ExternalAuth {
    /// The value to send on [`Self::header`] for `bearer`: `"{scheme}
    /// {token}"`, or the raw token when `scheme` is empty (providers that
    /// expect a bare credential, e.g. an `X-Api-Key`).
    pub fn header_value(&self, bearer: &str) -> String {
        if self.scheme.trim().is_empty() {
            bearer.to_owned()
        } else {
            format!("{} {bearer}", self.scheme)
        }
    }
}

impl Deployment {
    pub fn validate(&self, name: &str) -> Result<(), ConfigValidationError> {
        if matches!(self.server_type, McpServerType::Internal) {
            if let Some(ep) = self.endpoint.as_deref()
                && (ep.starts_with("http://") || ep.starts_with("https://"))
            {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{name}': endpoint must be a relative path (e.g. \
                         /api/v1/mcp/{name}/mcp) or omitted; the host is derived from \
                         server.api_external_url. Remove the scheme+host prefix."
                )));
            }
            if self.external_auth.is_some() || !self.headers.is_empty() {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{name}': external_auth and headers are only valid on \
                         external servers; internal servers are reached through the gateway \
                         with the systemprompt credential."
                )));
            }
        }

        if let Some(ext) = self.external_auth.as_ref() {
            if ext.token_endpoint.starts_with("http://")
                || ext.token_endpoint.starts_with("https://")
            {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{name}': external_auth.token_endpoint must be a relative \
                         path (e.g. /api/public/<provider>/token); the host is derived from \
                         server.api_external_url. Remove the scheme+host prefix."
                )));
            }
            if !ext.token_endpoint.starts_with('/') {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{name}': external_auth.token_endpoint must be an absolute \
                         path beginning with '/'."
                )));
            }
            if ext.header.trim().is_empty() {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{name}': external_auth.header must not be empty."
                )));
            }
        }

        Ok(())
    }
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
    pub client_id: Option<ClientId>,
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
    "/app".to_owned()
}
