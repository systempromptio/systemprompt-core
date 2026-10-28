//! Configuration of the generic `OpenID Connect` relying party
//! (`services/web/config/oidc.yaml`).
//!
//! A missing file is a provider that is off, never an error. An enabled file
//! is validated whole before the provider is built — issuer, client, scopes
//! and the presence of the client secret in the secrets store — so a broken
//! configuration is refused at boot rather than at the first sign-in. The
//! callback URL is never authored here: the driver derives it from the
//! profile's `api_external_url`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use systemprompt_models::net::{OutboundUrlError, validate_outbound_url};
use systemprompt_traits::SecretStore;
use systemprompt_traits::sso::SsoIdentityPolicy;

pub const CONFIG_FILE: &str = "web/config/oidc.yaml";

#[derive(Debug, thiserror::Error)]
pub enum OidcConfigError {
    #[error("{path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("OIDC `{0}` is required when the provider is enabled")]
    Missing(&'static str),
    #[error("OIDC `{field}` is not an acceptable outbound URL: {source}")]
    Url {
        field: &'static str,
        #[source]
        source: OutboundUrlError,
    },
    #[error("OIDC `scopes` must include `openid`")]
    NoOpenidScope,
    #[error("OIDC client secret `{key}` is missing from the secrets store")]
    MissingSecret { key: String },
    #[error("OIDC outbound HTTP client could not be built: {0}")]
    HttpClient(#[source] reqwest::Error),
}

fn default_client_secret_key() -> String {
    "oidc_client_secret".to_owned()
}

fn default_scopes() -> Vec<String> {
    ["openid", "profile", "email"].map(str::to_owned).to_vec()
}

const fn default_clock_skew_seconds() -> u64 {
    60
}

fn default_login_path() -> String {
    "/admin/login".to_owned()
}

fn default_return_to() -> String {
    "/admin".to_owned()
}

fn default_email_claim() -> String {
    "email".to_owned()
}

fn default_name_claim() -> String {
    "name".to_owned()
}

fn default_groups_claim() -> String {
    "groups".to_owned()
}

/// Which `id_token` claims carry the identity the policy reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcClaimMapping {
    #[serde(default = "default_email_claim")]
    pub email: String,
    #[serde(default = "default_name_claim")]
    pub name: String,
    #[serde(default = "default_groups_claim")]
    pub groups: String,
}

impl Default for OidcClaimMapping {
    fn default() -> Self {
        Self {
            email: default_email_claim(),
            name: default_name_claim(),
            groups: default_groups_claim(),
        }
    }
}

/// The generic OIDC relying party: one client at one issuer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub discovery_url: Option<String>,
    #[serde(default)]
    pub client_id: String,
    #[serde(default = "default_client_secret_key")]
    pub client_secret_key: String,
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub claims: OidcClaimMapping,
    #[serde(default)]
    pub jwks_allowed_hosts: Vec<String>,
    #[serde(default = "default_clock_skew_seconds")]
    pub clock_skew_seconds: u64,
    #[serde(default = "default_login_path")]
    pub login_path: String,
    #[serde(default = "default_return_to")]
    pub default_return_to: String,
    #[serde(default)]
    pub policy: SsoIdentityPolicy,
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            issuer: String::new(),
            discovery_url: None,
            client_id: String::new(),
            client_secret_key: default_client_secret_key(),
            scopes: default_scopes(),
            claims: OidcClaimMapping::default(),
            jwks_allowed_hosts: Vec::new(),
            clock_skew_seconds: default_clock_skew_seconds(),
            login_path: default_login_path(),
            default_return_to: default_return_to(),
            policy: SsoIdentityPolicy::default(),
        }
    }
}

impl OidcConfig {
    pub fn load(services_root: &Path) -> Result<Option<Self>, OidcConfigError> {
        let path = services_root.join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(OidcConfigError::Read { path, source }),
        };
        Self::parse(&text)
            .map(Some)
            .map_err(|source| OidcConfigError::Parse { path, source })
    }

    pub fn parse(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text)
    }

    #[must_use]
    pub fn issuer(&self) -> &str {
        self.issuer.trim_end_matches('/')
    }

    #[must_use]
    pub fn discovery_url(&self) -> String {
        self.discovery_url
            .clone()
            .unwrap_or_else(|| format!("{}/.well-known/openid-configuration", self.issuer()))
    }

    #[must_use]
    pub fn jwks_hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = url::Url::parse(self.issuer())
            .ok()
            .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
            .into_iter()
            .collect();
        for host in &self.jwks_allowed_hosts {
            let host = host.trim().to_ascii_lowercase();
            if !host.is_empty() && !hosts.contains(&host) {
                hosts.push(host);
            }
        }
        hosts
    }

    pub fn validate(&self, secrets: &dyn SecretStore) -> Result<(), OidcConfigError> {
        if self.issuer().trim().is_empty() {
            return Err(OidcConfigError::Missing("issuer"));
        }
        validate_outbound_url(self.issuer()).map_err(|source| OidcConfigError::Url {
            field: "issuer",
            source,
        })?;
        if let Some(discovery_url) = &self.discovery_url {
            validate_outbound_url(discovery_url).map_err(|source| OidcConfigError::Url {
                field: "discovery_url",
                source,
            })?;
        }
        if self.client_id.trim().is_empty() {
            return Err(OidcConfigError::Missing("client_id"));
        }
        if self.client_secret_key.trim().is_empty() {
            return Err(OidcConfigError::Missing("client_secret_key"));
        }
        if !self.scopes.iter().any(|scope| scope == "openid") {
            return Err(OidcConfigError::NoOpenidScope);
        }
        let secret_present = secrets
            .secret(&self.client_secret_key)
            .is_some_and(|secret| !secret.expose().is_empty());
        if !secret_present {
            return Err(OidcConfigError::MissingSecret {
                key: self.client_secret_key.clone(),
            });
        }
        Ok(())
    }
}
