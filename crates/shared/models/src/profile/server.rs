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
}
