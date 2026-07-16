//! Secrets document model.
//!
//! [`Secrets`] is the deserialized on-disk secrets file: OAuth at-rest
//! pepper, database URLs, and provider credentials.
//! [`OAUTH_AT_REST_PEPPER_MIN_LENGTH`] is the enforced minimum.
//! Validation returns [`crate::errors::SecretsError`].
//!
//! Secret hygiene is enforced by the type, not by convention: the hand-written
//! [`fmt::Debug`] redacts every credential so a stray `{:?}` or `?secrets`
//! cannot leak into logs, and the [`Drop`] impl wipes the plaintext fields from
//! memory via `zeroize`. `Serialize` is retained deliberately — operator
//! tooling round-trips the document back to the on-disk secrets file, the one
//! legitimate sink.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use zeroize::Zeroize;

use crate::errors::SecretsError;

pub const OAUTH_AT_REST_PEPPER_MIN_LENGTH: usize = 32;

#[derive(Clone, Serialize, Deserialize)]
pub struct Secrets {
    pub oauth_at_rest_pepper: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_signing_secret_seed: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key_pem: Option<String>,

    pub database_url: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_write_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_database_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_database_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moonshot: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qwen: Option<String>,

    #[serde(default, flatten)]
    pub custom: HashMap<String, String>,
}

impl fmt::Debug for Secrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secrets")
            .field("ai_providers", &self.has_ai_provider())
            .field("custom_keys", &self.custom.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl Drop for Secrets {
    fn drop(&mut self) {
        self.oauth_at_rest_pepper.zeroize();
        self.manifest_signing_secret_seed.zeroize();
        self.signing_key_pem.zeroize();
        self.database_url.zeroize();
        self.database_write_url.zeroize();
        self.external_database_url.zeroize();
        self.internal_database_url.zeroize();
        self.gemini.zeroize();
        self.anthropic.zeroize();
        self.openai.zeroize();
        self.github.zeroize();
        self.moonshot.zeroize();
        self.qwen.zeroize();
        for value in self.custom.values_mut() {
            value.zeroize();
        }
    }
}

impl Secrets {
    pub fn parse(content: &str) -> Result<Self, SecretsError> {
        let mut value: serde_json::Value =
            serde_json::from_str(content).map_err(|source| SecretsError::Parse {
                context: "Failed to parse secrets JSON",
                source,
            })?;
        // Null and blank entries are both "absent": setup wizards and container
        // platforms have historically persisted "" for unset provider keys, and
        // a blank key must not enable a provider.
        if let Some(obj) = value.as_object_mut() {
            obj.retain(|_, v| !v.is_null() && v.as_str().is_none_or(|s| !s.trim().is_empty()));
        }
        let secrets: Self =
            serde_json::from_value(value).map_err(|source| SecretsError::Parse {
                context: "Failed to deserialize secrets after null stripping",
                source,
            })?;
        secrets.validate()?;
        Ok(secrets)
    }

    pub fn validate(&self) -> Result<(), SecretsError> {
        if self.oauth_at_rest_pepper.len() < OAUTH_AT_REST_PEPPER_MIN_LENGTH {
            return Err(SecretsError::Invalid(format!(
                "oauth_at_rest_pepper must be at least {} characters (got {})",
                OAUTH_AT_REST_PEPPER_MIN_LENGTH,
                self.oauth_at_rest_pepper.len()
            )));
        }
        Ok(())
    }

    pub fn effective_database_url(&self, external_db_access: bool) -> &str {
        if external_db_access && let Some(url) = &self.external_database_url {
            return url;
        }
        &self.database_url
    }

    pub const fn has_ai_provider(&self) -> bool {
        self.gemini.is_some()
            || self.anthropic.is_some()
            || self.openai.is_some()
            || self.moonshot.is_some()
            || self.qwen.is_some()
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        match key {
            "oauth_at_rest_pepper" | "OAUTH_AT_REST_PEPPER" => Some(&self.oauth_at_rest_pepper),
            "signing_key_pem" | "SIGNING_KEY_PEM" => self.signing_key_pem.as_ref(),
            "database_url" | "DATABASE_URL" => Some(&self.database_url),
            "database_write_url" | "DATABASE_WRITE_URL" => self.database_write_url.as_ref(),
            "external_database_url" | "EXTERNAL_DATABASE_URL" => {
                self.external_database_url.as_ref()
            },
            "internal_database_url" | "INTERNAL_DATABASE_URL" => {
                self.internal_database_url.as_ref()
            },
            "gemini" | "GEMINI_API_KEY" => self.gemini.as_ref(),
            "anthropic" | "ANTHROPIC_API_KEY" => self.anthropic.as_ref(),
            "openai" | "OPENAI_API_KEY" => self.openai.as_ref(),
            "github" | "GITHUB_TOKEN" => self.github.as_ref(),
            "moonshot" | "MOONSHOT_API_KEY" | "kimi" | "KIMI_API_KEY" => self.moonshot.as_ref(),
            "qwen" | "QWEN_API_KEY" | "dashscope" | "DASHSCOPE_API_KEY" => self.qwen.as_ref(),
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

    pub fn to_subprocess_env(&self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = Vec::new();

        pairs.push((
            "OAUTH_AT_REST_PEPPER".to_owned(),
            self.oauth_at_rest_pepper.clone(),
        ));
        pairs.push(("DATABASE_URL".to_owned(), self.database_url.clone()));

        let optionals: &[(&str, &Option<String>)] = &[
            (
                "MANIFEST_SIGNING_SECRET_SEED",
                &self.manifest_signing_secret_seed,
            ),
            ("SIGNING_KEY_PEM", &self.signing_key_pem),
            ("DATABASE_WRITE_URL", &self.database_write_url),
            ("EXTERNAL_DATABASE_URL", &self.external_database_url),
            ("INTERNAL_DATABASE_URL", &self.internal_database_url),
            ("GEMINI_API_KEY", &self.gemini),
            ("ANTHROPIC_API_KEY", &self.anthropic),
            ("OPENAI_API_KEY", &self.openai),
            ("GITHUB_TOKEN", &self.github),
            ("MOONSHOT_API_KEY", &self.moonshot),
            ("QWEN_API_KEY", &self.qwen),
        ];
        for (name, value) in optionals {
            if let Some(v) = value
                && !v.is_empty()
            {
                pairs.push(((*name).to_owned(), v.clone()));
            }
        }

        if !self.custom.is_empty() {
            let mut names: Vec<String> = Vec::with_capacity(self.custom.len());
            for (key, value) in &self.custom {
                let upper = key.to_uppercase();
                names.push(upper.clone());
                pairs.push((upper.clone(), value.clone()));
                if upper != *key {
                    pairs.push((key.clone(), value.clone()));
                }
            }
            pairs.push(("SYSTEMPROMPT_CUSTOM_SECRETS".to_owned(), names.join(",")));
        }

        pairs
    }
}
