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

    #[serde(default)]
    pub security_headers: SecurityHeadersConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHeadersConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(default = "default_hsts")]
    pub hsts: String,

    #[serde(default = "default_frame_options")]
    pub frame_options: String,

    #[serde(default = "default_content_type_options")]
    pub content_type_options: String,

    #[serde(default = "default_referrer_policy")]
    pub referrer_policy: String,

    #[serde(default = "default_permissions_policy")]
    pub permissions_policy: String,

    #[serde(default)]
    pub content_security_policy: Option<String>,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hsts: default_hsts(),
            frame_options: default_frame_options(),
            content_type_options: default_content_type_options(),
            referrer_policy: default_referrer_policy(),
            permissions_policy: default_permissions_policy(),
            content_security_policy: None,
        }
    }
}

const fn default_enabled() -> bool {
    true
}

fn default_hsts() -> String {
    "max-age=63072000; includeSubDomains; preload".to_string()
}

fn default_frame_options() -> String {
    "DENY".to_string()
}

fn default_content_type_options() -> String {
    "nosniff".to_string()
}

fn default_referrer_policy() -> String {
    "strict-origin-when-cross-origin".to_string()
}

fn default_permissions_policy() -> String {
    "camera=(), microphone=(), geolocation=()".to_string()
}
