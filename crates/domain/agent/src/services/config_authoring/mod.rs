//! Authoring workflow for on-disk agent YAML configuration.
//!
//! [`AgentConfigAuthoringService`] owns the write path for
//! `services/agents/<name>.yaml`: input validation, shaping a full
//! [`AgentConfig`] from an [`AgentCreateRequest`], applying
//! [`AgentEditRequest`] mutations to an in-memory config, and deleting agent
//! files through [`ConfigWriter`]. An agent created without a model takes the
//! catalog's `gateway.default_model` (or the first model deployed on the
//! given provider), and its provider is the one that model's first enabled
//! deployment is served by. Interactive prompting, profile resolution,
//! and post-write configuration reloads stay with the caller. All failures
//! surface as [`ConfigAuthoringError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod edit;
mod set_value;

use std::fs;
use std::path::PathBuf;

use systemprompt_identifiers::AgentName;
use systemprompt_loader::{ConfigWriteError, ConfigWriter};
use systemprompt_manifest::services::catalog::{CatalogContext, ResolvedCatalog};
use systemprompt_manifest::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, CapabilitiesConfig, OAuthConfig,
};
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::plugin::PluginComponentRef;
use thiserror::Error;

pub use edit::AgentEditRequest;

#[derive(Debug, Error)]
pub enum ConfigAuthoringError {
    #[error("Agent name must be between 3 and 50 characters")]
    NameLength,

    #[error("Agent name must be lowercase alphanumeric with underscores only")]
    NameCharset,

    #[error("Port cannot be 0")]
    PortZero,

    #[error("Port must be >= 1024 (non-privileged)")]
    PortPrivileged,

    #[error("Failed to read system prompt file: {path}")]
    SystemPromptFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("MCP server '{name}' not found in configuration. Available servers: {available}")]
    UnknownMcpServer { name: String, available: String },

    #[error("Invalid --set format: '{0}'. Expected key=value")]
    InvalidSetFormat(String),

    #[error("Invalid boolean value for {key}: '{value}'")]
    InvalidBoolean { key: String, value: String },

    #[error(
        "Unknown configuration key: '{0}'. Supported keys: card.displayName, card.description, \
         card.version, endpoint, is_primary, default, dev_only"
    )]
    UnknownSetKey(String),

    #[error(transparent)]
    Write(#[from] ConfigWriteError),

    #[error("No model given and the catalog declares no gateway.default_model")]
    NoDefaultModel,

    #[error("No model given and the catalog deploys no model on provider '{0}'")]
    NoModelForProvider(String),

    #[error("Model '{0}' has no deployment on an enabled provider")]
    ModelNotServable(String),

    #[error("Failed to load services config for defaults: {0}")]
    ServicesConfig(#[from] systemprompt_loader::ConfigLoadError),

    #[error("The catalog does not resolve: {0}")]
    Catalog(#[from] systemprompt_manifest::services::catalog::CatalogError),
}

#[derive(Debug, Clone, Default)]
pub struct AgentCreateRequest {
    pub name: String,
    pub port: u16,
    pub display_name: String,
    pub description: String,
    pub system_prompt: String,
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub dev_only: bool,
    pub is_primary: bool,
    pub default: bool,
    pub version: Option<String>,
    pub icon_url: Option<String>,
    pub documentation_url: Option<String>,
    pub streaming: Option<bool>,
    pub push_notifications: Option<bool>,
    pub extended_agent_card: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub mcp_servers: Vec<String>,
    pub skills: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AgentConfigAuthoringService {
    services_dir: PathBuf,
}

impl AgentConfigAuthoringService {
    pub fn new(services_dir: impl Into<PathBuf>) -> Self {
        Self {
            services_dir: services_dir.into(),
        }
    }

    pub fn validate_agent_name(name: &str) -> Result<(), ConfigAuthoringError> {
        if name.len() < 3 || name.len() > 50 {
            return Err(ConfigAuthoringError::NameLength);
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(ConfigAuthoringError::NameCharset);
        }
        Ok(())
    }

    pub const fn validate_port(port: u16) -> Result<(), ConfigAuthoringError> {
        if port == 0 {
            return Err(ConfigAuthoringError::PortZero);
        }
        if port < 1024 {
            return Err(ConfigAuthoringError::PortPrivileged);
        }
        Ok(())
    }

    pub fn resolve_system_prompt(
        file: Option<&str>,
        inline: Option<String>,
        display_name: &str,
        description: &str,
    ) -> Result<String, ConfigAuthoringError> {
        if let Some(path) = file {
            return fs::read_to_string(path).map_err(|source| {
                ConfigAuthoringError::SystemPromptFile {
                    path: path.to_owned(),
                    source,
                }
            });
        }
        if let Some(prompt) = inline {
            return Ok(prompt);
        }
        Ok(if description.is_empty() {
            format!("You are {display_name}.")
        } else {
            format!("You are {display_name}. {description}")
        })
    }

    pub fn create(&self, request: AgentCreateRequest) -> Result<PathBuf, ConfigAuthoringError> {
        Self::validate_agent_name(&request.name)?;
        Self::validate_port(request.port)?;
        let agent_config = build_agent_config(request)?;
        Ok(ConfigWriter::create_agent(
            &agent_config,
            &self.services_dir,
        )?)
    }

    pub fn delete(&self, name: &AgentName) -> Result<(), ConfigAuthoringError> {
        Ok(ConfigWriter::delete_agent(name, &self.services_dir)?)
    }
}

fn build_agent_config(
    mut request: AgentCreateRequest,
) -> Result<AgentConfig, ConfigAuthoringError> {
    let (provider, model) = agent_defaults(request.provider.take(), request.model.take())?;
    let endpoint = match request.endpoint.take() {
        Some(endpoint) => endpoint,
        None => ApiPaths::agent_endpoint(&AgentName::new(&request.name)),
    };

    Ok(AgentConfig {
        name: request.name.clone(),
        port: request.port,
        endpoint,
        enabled: request.enabled,
        dev_only: request.dev_only,
        is_primary: request.is_primary,
        default: request.default,
        tags: Vec::new(),
        card: AgentCardConfig {
            protocol_version: crate::A2A_PROTOCOL_VERSION.to_owned(),
            name: Some(request.name),
            display_name: request.display_name,
            description: request.description,
            version: request.version.unwrap_or_else(|| "1.0.0".to_owned()),
            preferred_transport: "JSONRPC".to_owned(),
            icon_url: request.icon_url,
            documentation_url: request.documentation_url,
            provider: None,
            capabilities: CapabilitiesConfig {
                streaming: request.streaming.unwrap_or(true),
                push_notifications: request.push_notifications.unwrap_or(false),
                extended_agent_card: request.extended_agent_card.unwrap_or(false),
            },
            default_input_modes: vec!["text/plain".to_owned()],
            default_output_modes: vec!["text/plain".to_owned()],
            security_schemes: None,
            security_requirements: None,
        },
        metadata: AgentMetadataConfig {
            system_prompt: Some(request.system_prompt),
            mcp_servers: PluginComponentRef {
                include: request.mcp_servers,
                ..Default::default()
            },
            skills: PluginComponentRef {
                include: request.skills,
                ..Default::default()
            },
            provider: Some(provider),
            model: Some(model),
            ..Default::default()
        },
        oauth: OAuthConfig::default(),
    })
}

fn agent_defaults(
    provider: Option<String>,
    model: Option<String>,
) -> Result<(String, String), ConfigAuthoringError> {
    if let (Some(provider), Some(model)) = (provider.as_ref(), model.as_ref()) {
        return Ok((provider.clone(), model.clone()));
    }
    let services = systemprompt_loader::ConfigLoader::load()?;
    let catalog = services.resolve_catalog(&CatalogContext::default())?;
    let model = match (model, provider.as_deref()) {
        (Some(model), _) => model,
        (None, Some(provider)) => model_on_provider(&catalog, provider)?,
        (None, None) => catalog
            .default_model()
            .map(|m| m.as_str().to_owned())
            .ok_or(ConfigAuthoringError::NoDefaultModel)?,
    };
    let provider = match provider {
        Some(provider) => provider,
        None => catalog
            .enabled_hops(&model)
            .next()
            .map(|hop| hop.provider.as_str().to_owned())
            .ok_or_else(|| ConfigAuthoringError::ModelNotServable(model.clone()))?,
    };
    Ok((provider, model))
}

fn model_on_provider(
    catalog: &ResolvedCatalog,
    provider: &str,
) -> Result<String, ConfigAuthoringError> {
    let primary_on = |model: &str| {
        catalog
            .enabled_hops(model)
            .next()
            .is_some_and(|hop| hop.provider.as_str() == provider)
    };
    catalog
        .default_model()
        .map(|m| m.as_str().to_owned())
        .filter(|m| primary_on(m))
        .or_else(|| {
            catalog
                .models()
                .map(|m| m.id.as_str().to_owned())
                .find(|m| primary_on(m))
        })
        .ok_or_else(|| ConfigAuthoringError::NoModelForProvider(provider.to_owned()))
}
