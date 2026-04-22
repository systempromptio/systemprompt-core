use serde::Deserialize;
use std::path::PathBuf;
use std::{env, fs};

const DEFAULT_GATEWAY_URL: &str = "http://localhost:8080";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default = "default_gateway_url")]
    pub gateway_url: String,
    #[serde(default)]
    pub pat: Option<PatConfig>,
    #[serde(default)]
    pub session: Option<SessionConfig>,
    #[serde(default)]
    pub mtls: Option<MtlsConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PatConfig {
    #[serde(default)]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionConfig {
    #[serde(default)]
    pub keystore_service: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MtlsConfig {
    #[serde(default)]
    pub cert_keystore_ref: Option<String>,
}

fn default_gateway_url() -> String {
    DEFAULT_GATEWAY_URL.to_string()
}

pub fn load() -> Config {
    let path = config_path();
    let mut cfg: Config = path
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    if let Ok(url) = env::var("SP_COWORK_GATEWAY_URL") {
        cfg.gateway_url = url;
    }
    if cfg.gateway_url.is_empty() {
        cfg.gateway_url = DEFAULT_GATEWAY_URL.to_string();
    }
    cfg
}

fn config_path() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("SP_COWORK_CONFIG") {
        return Some(PathBuf::from(explicit));
    }
    let base = dirs::config_dir()?;
    Some(base.join("systemprompt").join("cowork-auth.toml"))
}
