use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

pub(crate) static SECRETS: OnceLock<Secrets> = OnceLock::new();

pub(crate) const JWT_SECRET_MIN_LENGTH: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secrets {
    pub jwt_secret: String,

    pub database_url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_write_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_database_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_database_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_token: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,

    #[serde(default, flatten)]
    pub custom: HashMap<String, String>,
}

impl Secrets {
    pub fn parse(content: &str) -> Result<Self> {
        let secrets: Self =
            serde_json::from_str(content).context("Failed to parse secrets JSON")?;
        secrets.validate()?;
        Ok(secrets)
    }

    pub fn load_from_path(secrets_path: &Path) -> Result<Self> {
        if !secrets_path.exists() {
            anyhow::bail!("Secrets file not found: {}", secrets_path.display());
        }
        let content = std::fs::read_to_string(secrets_path)
            .with_context(|| format!("Failed to read secrets: {}", secrets_path.display()))?;
        Self::parse(&content)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.jwt_secret.len() < JWT_SECRET_MIN_LENGTH {
            anyhow::bail!(
                "jwt_secret must be at least {} characters (got {})",
                JWT_SECRET_MIN_LENGTH,
                self.jwt_secret.len()
            );
        }
        Ok(())
    }

    pub fn effective_database_url(&self, external_db_access: bool) -> &str {
        if external_db_access {
            if let Some(url) = &self.external_database_url {
                return url;
            }
        }
        &self.database_url
    }

    pub const fn has_ai_provider(&self) -> bool {
        self.gemini.is_some() || self.anthropic.is_some() || self.openai.is_some()
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        match key {
            "jwt_secret" | "JWT_SECRET" => Some(&self.jwt_secret),
            "database_url" | "DATABASE_URL" => Some(&self.database_url),
            "database_write_url" | "DATABASE_WRITE_URL" => self.database_write_url.as_ref(),
            "external_database_url" | "EXTERNAL_DATABASE_URL" => {
                self.external_database_url.as_ref()
            },
            "internal_database_url" | "INTERNAL_DATABASE_URL" => {
                self.internal_database_url.as_ref()
            },
            "sync_token" | "SYNC_TOKEN" => self.sync_token.as_ref(),
            "gemini" | "GEMINI_API_KEY" => self.gemini.as_ref(),
            "anthropic" | "ANTHROPIC_API_KEY" => self.anthropic.as_ref(),
            "openai" | "OPENAI_API_KEY" => self.openai.as_ref(),
            "github" | "GITHUB_TOKEN" => self.github.as_ref(),
            other => self.custom.get(other).or_else(|| {
                let alternate = if other.chars().any(char::is_uppercase) {
                    other.to_lowercase()
                } else {
                    other.to_uppercase()
                };
                self.custom.get(&alternate)
            }),
        }
    }

    pub fn log_configured_providers(&self) {
        let configured: Vec<&str> = [
            self.gemini.as_ref().map(|_| "gemini"),
            self.anthropic.as_ref().map(|_| "anthropic"),
            self.openai.as_ref().map(|_| "openai"),
            self.github.as_ref().map(|_| "github"),
        ]
        .into_iter()
        .flatten()
        .collect();

        tracing::info!(providers = ?configured, "Configured API providers");
    }

    pub fn custom_env_vars(&self) -> Vec<(String, &str)> {
        self.custom
            .iter()
            .flat_map(|(key, value)| {
                let upper_key = key.to_uppercase();
                let value_str = value.as_str();
                if upper_key == *key {
                    vec![(key.clone(), value_str)]
                } else {
                    vec![(key.clone(), value_str), (upper_key, value_str)]
                }
            })
            .collect()
    }

    pub fn custom_env_var_names(&self) -> Vec<String> {
        self.custom.keys().map(|key| key.to_uppercase()).collect()
    }
}
