use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub servers: Vec<super::server::McpServerConfig>,
    pub registry_url: Option<String>,
    pub cache_dir: Option<String>,
}
