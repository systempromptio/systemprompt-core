//! Bridge configuration: gateway URL, profile, and runtime settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod paths;
mod profile;
pub mod redaction;
mod runtime;
pub mod store;
pub mod trust;
pub mod write;

pub use runtime::{RuntimeConfig, SharedRuntimeConfig, shared_from_config, shared_from_loaded};

use serde::Deserialize;
use std::env;
use std::path::PathBuf;

use systemprompt_identifiers::ValidatedUrl;

use crate::ids::KeystoreRef;

pub use self::profile::{ClaudeConfig, gateway_url_or_default};
pub use self::trust::{
    LegacyPin, PinSource, PinnedPubkeyState, SyncConfig, TrustError, persist_pinned_pubkey,
    pinned_pubkey, pinned_pubkey_state, policy_pubkey,
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
    pub mtls: Option<MtlsConfig>,
    #[serde(default)]
    pub sync: Option<SyncConfig>,
    #[serde(default)]
    pub claude: Option<ClaudeConfig>,
    #[serde(default)]
    pub cowork: Option<CoworkConfig>,
    #[serde(default)]
    pub deployment_organization_uuid: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoworkConfig {
    #[serde(default)]
    pub session_org_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PatConfig {
    #[serde(default)]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct SessionConfig {
    #[serde(default)]
    pub generation: Option<uuid::Uuid>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MtlsConfig {
    #[serde(default)]
    pub cert_keystore_ref: Option<KeystoreRef>,
}

impl Config {
    pub fn load() -> Result<Self, ConfigReadError> {
        let mut cfg = read()?;
        if cfg.gateway_url.is_none() {
            cfg.gateway_url = Some(default_gateway());
        }
        Ok(cfg)
    }

    #[must_use]
    pub fn cert_keystore_ref(&self) -> Option<&KeystoreRef> {
        self.mtls
            .as_ref()
            .and_then(|m| m.cert_keystore_ref.as_ref())
    }
}

pub fn load() -> Result<Config, ConfigReadError> {
    Config::load()
}

#[must_use]
pub fn config_path() -> Option<PathBuf> {
    if let Ok(explicit) = env::var(crate::brand::brand().env("CONFIG")) {
        return Some(PathBuf::from(explicit));
    }
    let base = crate::basedirs::config_dir()?;
    let brand = crate::brand::brand();
    Some(base.join(brand.config_dir).join(brand.config_file))
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigReadError {
    #[error("config path unresolvable on this platform")]
    PathUnresolvable,
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
    let path = config_path().ok_or(ConfigReadError::PathUnresolvable)?;
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

pub fn ensure_gateway_url(url: &str) -> Result<(), ConfigWriteError> {
    write::edit(|doc| write::set_if_absent(doc, &["gateway_url"], url))
}
