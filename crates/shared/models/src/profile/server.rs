//! Server configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
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

    /// Stable identifier for this replica. Empty/unset resolves to the
    /// OS hostname (or a generated short id) at config build time.
    #[serde(default)]
    pub instance_id: Option<String>,

    /// Global cap on concurrent A2A SSE streams for this replica.
    #[serde(default = "default_max_concurrent_streams")]
    pub max_concurrent_streams: usize,

    /// CIDR ranges whose immediate-peer requests are allowed to set
    /// `X-Forwarded-For`, `X-Real-IP`, and `CF-Connecting-IP`. Empty
    /// means the platform treats every connection as direct and ignores
    /// those headers — the only safe default behind no proxy. Each entry
    /// is a CIDR string (e.g. `10.0.0.0/8`, `192.168.1.0/24`,
    /// `2001:db8::/32`). Single addresses without a `/` are accepted as
    /// `/32` (IPv4) or `/128` (IPv6).
    #[serde(default)]
    pub trusted_proxies: Vec<String>,
}

const fn default_max_concurrent_streams() -> usize {
    crate::config::DEFAULT_MAX_CONCURRENT_STREAMS
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
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
    ".md".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
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
    "max-age=63072000; includeSubDomains; preload".to_owned()
}

fn default_frame_options() -> String {
    "DENY".to_owned()
}

fn default_content_type_options() -> String {
    "nosniff".to_owned()
}

fn default_referrer_policy() -> String {
    "strict-origin-when-cross-origin".to_owned()
}

fn default_permissions_policy() -> String {
    "camera=(), microphone=(), geolocation=()".to_owned()
}
