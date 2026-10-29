//! Bridge configuration: gateway URL, profile, and runtime settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod dir_owner;
pub mod gateways;
pub mod paths;
mod profile;
pub mod redaction;
mod runtime;
pub mod store;
pub mod trust;
pub mod write;

pub use runtime::{RuntimeConfig, SharedRuntimeConfig, shared_from_config, shared_from_loaded};

use serde::Deserialize;
use std::path::PathBuf;

use systemprompt_identifiers::ValidatedUrl;


pub use self::profile::{ClaudeConfig, gateway_url_or_default};
pub use self::trust::{
    GatewayIdentity, PinSource, PinnedPubkeyState, SyncConfig, TrustError, TrustRecord,
    persist_pinned_pubkey, pinned_pubkey, pinned_pubkey_state,
};
pub use self::write::ConfigWriteError;

pub(crate) fn default_gateway() -> ValidatedUrl {
    ValidatedUrl::try_new(crate::brand::brand().default_gateway_url).unwrap_or_else(|_| {
        crate::stdio::diag("config: brand default_gateway_url failed validation");
        std::process::abort()
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub gateway_url: Option<ValidatedUrl>,
    #[serde(default)]
    pub pat: Option<PatConfig>,
    #[serde(default)]
    pub session: Option<SessionConfig>,
    #[serde(default)]
    pub sync: Option<SyncConfig>,
    #[serde(default)]
    pub claude: Option<ClaudeConfig>,
    #[serde(default)]
    pub cowork: Option<CoworkConfig>,
    #[serde(default)]
    pub opencode: Option<OpenCodeConfig>,
    #[serde(default)]
    pub drive: Option<DriveConfig>,
    #[serde(default)]
    pub instance: Option<InstanceConfig>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct InstanceConfig {
    #[serde(default)]
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct DriveConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpenCodeConfig {
    #[serde(default)]
    pub managed_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoworkConfig {
    #[serde(default)]
    pub session_org_dir: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct PatConfig {
    #[serde(default)]
    pub store: Option<gateways::credential::PatStore>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct SessionConfig {
    #[serde(default)]
    pub generation: Option<uuid::Uuid>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

impl Config {
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.instance.and_then(|i| i.disabled).unwrap_or(false)
    }

    pub fn load() -> Result<Self, ConfigReadError> {
        let mut cfg = read()?;
        if cfg.gateway_url.is_none() {
            cfg.gateway_url = Some(default_gateway());
        }
        Ok(cfg)
    }
}

pub fn load() -> Result<Config, ConfigReadError> {
    Config::load()
}

#[must_use]
pub fn config_path() -> Option<PathBuf> {
    gateways::active().ok().map(|profile| profile.config_file)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigReadError {
    #[error("config path unresolvable on this platform")]
    PathUnresolvable,
    #[error("gateway profile: {0}")]
    Profile(#[source] Box<gateways::ProfileError>),
    #[error("read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not valid TOML: {source}")]
    Malformed {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
}

pub fn read() -> Result<Config, ConfigReadError> {
    let profile = gateways::active().map_err(|e| ConfigReadError::Profile(Box::new(e)))?;
    read_file(&profile.config_file)
}

pub fn read_file(path: &std::path::Path) -> Result<Config, ConfigReadError> {
    let path = path.to_path_buf();
    let Some(body) =
        crate::fsutil::read_optional(&path).map_err(|source| ConfigReadError::Read {
            path: path.clone(),
            source,
        })?
    else {
        return Ok(Config::default());
    };
    toml::from_str(&body).map_err(|source| ConfigReadError::Malformed {
        path,
        source: Box::new(source),
    })
}

#[must_use]
pub fn same_origin(a: &ValidatedUrl, b: &ValidatedUrl) -> bool {
    match (url::Url::parse(a.as_str()), url::Url::parse(b.as_str())) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => a.as_str().trim_end_matches('/') == b.as_str().trim_end_matches('/'),
    }
}

pub fn ensure_gateway_url(url: &str) -> Result<(), ConfigWriteError> {
    write::edit(|doc| write::set_if_absent(doc, &["gateway_url"], url))
}
