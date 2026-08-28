//! Bridge configuration: gateway URL, profile, and runtime settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod paths;
mod profile;
pub mod redaction;
mod runtime;
pub mod store;
pub mod write;

pub use runtime::{RuntimeConfig, SharedRuntimeConfig, shared_from_loaded};

use serde::Deserialize;
use std::env;
use std::path::PathBuf;
use std::sync::{LazyLock, Once};

use systemprompt_identifiers::ValidatedUrl;

use crate::ids::{KeystoreRef, PinnedPubKey};

pub use self::profile::{
    ClaudeConfig, gateway_url_or_default, persist_pinned_pubkey, pinned_pubkey, policy_pubkey,
};
pub use self::write::ConfigWriteError;

static DEFAULT_GATEWAY: LazyLock<ValidatedUrl> = LazyLock::new(|| {
    ValidatedUrl::try_new(crate::brand::brand().default_gateway_url).unwrap_or_else(|_| {
        crate::obs::output::diag("config: brand default_gateway_url failed validation");
        std::process::abort()
    })
});

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
    pub update: Option<UpdateConfig>,
    #[serde(default)]
    pub deployment_organization_uuid: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct UpdateConfig {
    #[serde(default)]
    pub automatic: Option<bool>,
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
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MtlsConfig {
    #[serde(default)]
    pub cert_keystore_ref: Option<KeystoreRef>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub pinned_pubkey: Option<PinnedPubKey>,
}

impl Config {
    pub fn load() -> Self {
        let mut cfg = match read() {
            Ok(cfg) => cfg,
            Err(e) => {
                static WARN_ONCE: Once = Once::new();
                WARN_ONCE.call_once(|| {
                    tracing::warn!(error = %e, "config unreadable; falling back to defaults");
                });
                Self::default()
            },
        };

        if cfg.gateway_url.is_none() {
            cfg.gateway_url = Some(DEFAULT_GATEWAY.clone());
        }

        cfg
    }

    #[must_use]
    pub fn with_policy_overrides(mut self) -> Self {
        let Some(policy_value) = policy_pubkey() else {
            return self;
        };
        let sync = self.sync.get_or_insert_with(SyncConfig::default);
        match sync.pinned_pubkey.as_ref() {
            None => sync.pinned_pubkey = Some(policy_value),
            Some(existing) if existing.as_str() == policy_value.as_str() => {},
            Some(existing) => {
                static WARN_ONCE: Once = Once::new();
                let existing_prefix: String = existing.as_str().chars().take(12).collect();
                let policy_prefix: String = policy_value.as_str().chars().take(12).collect();
                WARN_ONCE.call_once(|| {
                    tracing::warn!(
                        operator_pubkey_prefix = %existing_prefix,
                        policy_pubkey_prefix = %policy_prefix,
                        "policy-provided manifest pubkey overrides operator-set value"
                    );
                });
                sync.pinned_pubkey = Some(policy_value);
            },
        }
        self
    }

    #[must_use]
    pub fn cert_keystore_ref(&self) -> Option<&KeystoreRef> {
        self.mtls
            .as_ref()
            .and_then(|m| m.cert_keystore_ref.as_ref())
    }
}

#[must_use]
pub fn load() -> Config {
    Config::load().with_policy_overrides()
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
        source: toml::de::Error,
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
    toml::from_str(&body).map_err(|source| ConfigReadError::Malformed { path, source })
}

pub fn ensure_gateway_url(url: &str) -> Result<(), ConfigWriteError> {
    write::edit(|doc| write::set_if_absent(doc, &["gateway_url"], url))
}

pub fn set_update_automatic(enabled: bool) -> Result<(), ConfigWriteError> {
    write::edit(|doc| write::set(doc, &["update", "automatic"], enabled))
}

pub fn set_session_enabled(enabled: bool) -> Result<(), ConfigWriteError> {
    write::edit(|doc| write::set(doc, &["session", "enabled"], enabled))
}
