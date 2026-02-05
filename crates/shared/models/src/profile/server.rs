//! Server configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,

    pub port: u16,

    pub api_server_url: String,

    pub api_internal_url: String,

    pub api_external_url: String,

    #[serde(default)]
    pub use_https: bool,

    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,

    #[serde(default)]
    pub content_negotiation: ContentNegotiationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentNegotiationConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_markdown_suffix")]
    pub markdown_suffix: String,
}

impl Default for ContentNegotiationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            markdown_suffix: default_markdown_suffix(),
        }
    }
}

fn default_markdown_suffix() -> String {
    ".md".to_string()
}
