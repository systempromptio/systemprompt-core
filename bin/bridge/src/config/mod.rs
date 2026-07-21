//! Bridge configuration: gateway URL, profile, and runtime settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod paths;
mod profile;
pub mod redaction;
mod runtime;
pub mod store;

pub use runtime::{RuntimeConfig, SharedRuntimeConfig, shared_from_loaded};

use serde::Deserialize;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::{LazyLock, Once};
use std::{env, fs};

use systemprompt_identifiers::ValidatedUrl;

use crate::ids::{KeystoreRef, PinnedPubKey};

pub use self::profile::{
    ClaudeConfig, gateway_url_or_default, persist_pinned_pubkey, pinned_pubkey, policy_pubkey,
};

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
        let path = config_path();
        let mut cfg: Self = path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();

        if let Ok(url) = env::var(crate::brand::brand().env("GATEWAY_URL"))
            && let Ok(parsed) = ValidatedUrl::try_new(url.trim())
        {
            cfg.gateway_url = Some(parsed);
        }
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
    let base = dirs::config_dir()?;
    let brand = crate::brand::brand();
    Some(base.join(brand.config_dir).join(brand.config_file))
}

pub fn ensure_gateway_url(url: &str) -> std::io::Result<()> {
    let path = config_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "config path unresolvable")
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.contains("gateway_url") {
        return Ok(());
    }
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    _ = writeln!(next, "gateway_url = \"{url}\"");
    fs::write(&path, next)
}
