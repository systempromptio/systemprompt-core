use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ai::ToolModelConfig;
use crate::auth::{AuthenticatedUser, Permission};

pub const RUNNING: &str = "running";
pub const ERROR: &str = "error";
pub const STOPPED: &str = "stopped";
pub const STARTING: &str = "starting";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub binary: String,
    pub enabled: bool,
    pub display_in_web: bool,
    pub port: u16,
    #[serde(
        serialize_with = "serialize_path",
        deserialize_with = "deserialize_path"
    )]
    pub crate_path: PathBuf,
    pub display_name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub schemas: Vec<super::deployment::SchemaDefinition>,
    pub oauth: super::deployment::OAuthRequirement,
    #[serde(default)]
    pub tools: HashMap<String, super::deployment::ToolMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<ToolModelConfig>,
    #[serde(default)]
    pub env_vars: Vec<String>,
    pub version: String,
    pub host: String,
    pub module_name: String,
    pub protocol: String,
}

fn serialize_path<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    path.to_string_lossy().serialize(serializer)
}

fn deserialize_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(PathBuf::from(s))
}

impl McpServerConfig {
    pub fn endpoint(&self, api_server_url: &str) -> String {
        format!("{}/api/v1/mcp/{}/mcp", api_server_url, self.name)
    }

    pub fn from_manifest_and_deployment(
        name: String,
        manifest: &super::registry::ServerManifest,
        deployment: &super::deployment::Deployment,
        crate_path: PathBuf,
    ) -> Self {
        Self {
            name,
            binary: deployment.binary.clone(),
            enabled: deployment.enabled,
            display_in_web: deployment.display_in_web,
            port: deployment.port,
            crate_path,
            display_name: manifest.name.clone(),
            description: manifest.description.clone(),
            capabilities: vec!["tools".to_string(), "prompts".to_string()],
            schemas: deployment.schemas.clone(),
            oauth: deployment.oauth.clone(),
            tools: deployment.tools.clone(),
            model_config: deployment.model_config.clone(),
            env_vars: deployment.env_vars.clone(),
            version: manifest.version.clone(),
            host: "127.0.0.1".to_string(),
            module_name: "mcp".to_string(),
            protocol: "mcp".to_string(),
        }
    }
}

/// Authentication state for MCP connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpAuthState {
    Authenticated(AuthenticatedUser),
    Anonymous,
}

impl McpAuthState {
    pub const fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated(_))
    }

    pub const fn is_anonymous(&self) -> bool {
        matches!(self, Self::Anonymous)
    }

    pub const fn user(&self) -> Option<&AuthenticatedUser> {
        match self {
            Self::Authenticated(user) => Some(user),
            Self::Anonymous => None,
        }
    }

    pub fn has_permission(&self, permission: Permission) -> bool {
        match self {
            Self::Authenticated(user) => user.has_permission(permission),
            Self::Anonymous => permission == Permission::Anonymous,
        }
    }

    pub fn username(&self) -> String {
        match self {
            Self::Authenticated(user) => user.username.clone(),
            Self::Anonymous => "anonymous".to_string(),
        }
    }
}
