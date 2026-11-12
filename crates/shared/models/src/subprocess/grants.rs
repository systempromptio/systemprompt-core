//! The secrets a spawned MCP server or agent may receive.
//!
//! A child receives the database URLs and the public token verification keys
//! and nothing else, unless its manifest entry names a [`SecretGrant`] under
//! `secrets:`. A grant is one of the AI provider keys, the encryption master
//! key, the profile's storage credential, or a named custom secret. The OAuth
//! at-rest pepper, the manifest signing seed, the token signing keys and the
//! licence signing secret are held by the API process only and are refused as
//! grants, whether named directly or as `custom:<name>`.
//!
//! [`is_api_held_env_name`] recognises the environment names that carry a
//! secret or lead to one (the profile path, which names the secrets source
//! and the signing key files, and the default Vault bootstrap credentials),
//! so a manifest cannot route one through `env_vars` around the grant list.
//! It is the profile-independent floor: a renamed Vault credential is
//! refused by the profile-aware check in `systemprompt_manifest::child_env`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use serde::{Deserialize, Serialize};

const CUSTOM_PREFIX: &str = "custom:";

const API_HELD: &[&str] = &[
    "oauth_at_rest_pepper",
    "manifest_signing_secret_seed",
    "signing_key_pem",
    "signing_key_staged_pem",
    "signing_key_previous_keys",
    "licence_signing_secret",
];

/// An AI provider whose API key a child may be granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProviderSecret {
    Anthropic,
    Openai,
    Gemini,
    Moonshot,
    Qwen,
    Github,
}

impl ProviderSecret {
    pub const ALL: [Self; 6] = [
        Self::Anthropic,
        Self::Openai,
        Self::Gemini,
        Self::Moonshot,
        Self::Qwen,
        Self::Github,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::Gemini => "gemini",
            Self::Moonshot => "moonshot",
            Self::Qwen => "qwen",
            Self::Github => "github",
        }
    }

    #[must_use]
    pub const fn env_name(self) -> &'static str {
        match self {
            Self::Anthropic => "ANTHROPIC_API_KEY",
            Self::Openai => "OPENAI_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
            Self::Moonshot => "MOONSHOT_API_KEY",
            Self::Qwen => "QWEN_API_KEY",
            Self::Github => "GITHUB_TOKEN",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|provider| provider.as_str() == name)
    }

    #[must_use]
    pub fn from_provider_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "anthropic" | "claude" => Some(Self::Anthropic),
            "openai" | "openai-responses" => Some(Self::Openai),
            "gemini" | "google" => Some(Self::Gemini),
            "moonshot" | "kimi" => Some(Self::Moonshot),
            "qwen" | "dashscope" => Some(Self::Qwen),
            _ => None,
        }
    }
}

/// One secret a manifest grants to a spawned child.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum SecretGrant {
    Provider(ProviderSecret),
    EncryptionMasterKey,
    Storage,
    Custom(String),
}

impl SecretGrant {
    #[must_use]
    pub fn env_name(&self) -> String {
        match self {
            Self::Provider(provider) => provider.env_name().to_owned(),
            Self::EncryptionMasterKey => "ENCRYPTION_MASTER_KEY".to_owned(),
            Self::Storage => "STORAGE".to_owned(),
            Self::Custom(name) => name.to_ascii_uppercase(),
        }
    }

    #[must_use]
    pub fn secret_key(&self) -> &str {
        match self {
            Self::Provider(provider) => provider.as_str(),
            Self::EncryptionMasterKey => "encryption_master_key",
            Self::Storage => "storage",
            Self::Custom(name) => name,
        }
    }
}

impl SecretGrant {
    pub fn from_secret_name(name: &str) -> Result<Self, SecretGrantError> {
        format!("{CUSTOM_PREFIX}{name}").parse()
    }
}

impl fmt::Display for SecretGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(provider) => f.write_str(provider.as_str()),
            Self::EncryptionMasterKey => f.write_str("encryption_master_key"),
            Self::Storage => f.write_str("storage"),
            Self::Custom(name) => write!(f, "{CUSTOM_PREFIX}{name}"),
        }
    }
}

impl From<SecretGrant> for String {
    fn from(grant: SecretGrant) -> Self {
        grant.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretGrantError {
    #[error(
        "secret '{0}' is held by the API process only and cannot be granted to a spawned server \
         or agent"
    )]
    ApiHeld(String),
    #[error(
        "secret '{0}' is not grantable: name a provider (anthropic, openai, gemini, moonshot, \
         qwen, github), encryption_master_key, storage, or custom:<name>"
    )]
    Unknown(String),
    #[error("custom secret name '{0}' must be non-empty lowercase ASCII letters, digits or '_'")]
    InvalidCustomName(String),
    #[error(
        "the database URLs and verification keys are granted to every child; remove '{0}' from \
         secrets"
    )]
    AlwaysGranted(String),
}

impl TryFrom<String> for SecretGrant {
    type Error = SecretGrantError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        raw.parse()
    }
}

impl std::str::FromStr for SecretGrant {
    type Err = SecretGrantError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let raw = raw.trim();
        if let Some(name) = raw.strip_prefix(CUSTOM_PREFIX) {
            let name = name.to_ascii_lowercase();
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(SecretGrantError::InvalidCustomName(name));
            }
            if API_HELD.contains(&name.as_str()) {
                return Err(SecretGrantError::ApiHeld(name));
            }
            if name == "encryption_master_key" {
                return Ok(Self::EncryptionMasterKey);
            }
            return Ok(ProviderSecret::from_name(&name).map_or(Self::Custom(name), Self::Provider));
        }
        let name = raw.to_ascii_lowercase();
        if API_HELD.contains(&name.as_str()) {
            return Err(SecretGrantError::ApiHeld(name));
        }
        if name.starts_with("database") || name.ends_with("database_url") {
            return Err(SecretGrantError::AlwaysGranted(name));
        }
        if name == "encryption_master_key" {
            return Ok(Self::EncryptionMasterKey);
        }
        if name == "storage" {
            return Ok(Self::Storage);
        }
        ProviderSecret::from_name(&name)
            .map(Self::Provider)
            .ok_or(SecretGrantError::Unknown(name))
    }
}

#[must_use]
pub fn is_api_held_env_name(name: &str) -> bool {
    let upper = name.trim().to_ascii_uppercase();
    upper.ends_with("_API_KEY")
        || upper == "GITHUB_TOKEN"
        || upper.starts_with("DATABASE_")
        || upper.ends_with("_DATABASE_URL")
        || upper.starts_with("SIGNING_KEY_")
        || upper == "ENCRYPTION_MASTER_KEY"
        || upper == "SYSTEMPROMPT_CUSTOM_SECRETS"
        || upper == "SYSTEMPROMPT_PROFILE"
        || upper.starts_with("VAULT_")
        || API_HELD
            .iter()
            .any(|held| held.eq_ignore_ascii_case(upper.as_str()))
}
