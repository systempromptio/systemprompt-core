//! The failures of the agent configuration authoring workflow.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_loader::ConfigWriteError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigAuthoringError {
    #[error("Agent name must be between 3 and 50 characters")]
    NameLength,

    #[error("Agent name must be lowercase alphanumeric with underscores only")]
    NameCharset,

    #[error("Agent name '{name}' is reserved")]
    NameReserved { name: String },

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

    #[error("Invalid --set format: '{value}'. Expected key=value")]
    InvalidSetFormat { value: String },

    #[error("Invalid boolean value for {key}: '{value}'")]
    InvalidBoolean { key: String, value: String },

    #[error(
        "Unknown configuration key: '{key}'. Supported keys: card.displayName, card.description, \
         card.version, endpoint, is_primary, default, dev_only"
    )]
    UnknownSetKey { key: String },

    #[error(transparent)]
    Write(#[from] ConfigWriteError),

    #[error("No model given and the catalog declares no gateway.default_model")]
    NoDefaultModel,

    #[error("No model given and core's default catalog deploys no model on provider '{provider}'")]
    NoModelForProvider { provider: String },

    #[error("Model '{model}' has no deployment on an enabled provider")]
    ModelNotServable { model: String },

    #[error("Failed to load services config for defaults: {0}")]
    ServicesConfig(#[from] systemprompt_loader::ConfigLoadError),

    #[error("The catalog does not resolve: {0}")]
    Catalog(#[from] systemprompt_manifest::services::catalog::CatalogError),

    #[error("Core's default catalog is unreadable: {0}")]
    DefaultCatalog(#[from] systemprompt_manifest::services::catalog::CatalogDocumentError),
}
