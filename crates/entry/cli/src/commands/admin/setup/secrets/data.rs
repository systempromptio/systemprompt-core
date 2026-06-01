//! Secrets data model and default-provider resolution.
//!
//! [`SecretsData`] holds the generated OAuth at-rest pepper, database URL, and
//! AI-provider keys. [`resolve_primary`] picks the default provider from an
//! explicit flag or the first present key by [`PROVIDER_PRIORITY`].

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ProviderId;

use super::super::SetupArgs;

pub(super) const STANDARD_PROVIDERS: [&str; 3] = ["gemini", "anthropic", "openai"];

// Default-provider precedence when no --default-provider flag is given: the
// first provider in this order whose key was supplied wins.
const PROVIDER_PRIORITY: [&str; 3] = ["anthropic", "openai", "gemini"];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SecretsData {
    pub oauth_at_rest_pepper: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
}

impl SecretsData {
    pub(crate) const fn has_ai_provider(&self) -> bool {
        self.gemini.is_some() || self.anthropic.is_some() || self.openai.is_some()
    }

    fn key_for(&self, provider: &str) -> Option<&String> {
        match provider {
            "gemini" => self.gemini.as_ref(),
            "anthropic" => self.anthropic.as_ref(),
            "openai" => self.openai.as_ref(),
            _ => None,
        }
    }

    pub(crate) fn present_providers(&self) -> Vec<&'static str> {
        STANDARD_PROVIDERS
            .into_iter()
            .filter(|p| self.key_for(p).is_some())
            .collect()
    }

    pub(crate) fn summary(&self) -> String {
        let mut keys = Vec::new();
        if self.gemini.is_some() {
            keys.push("Gemini");
        }
        if self.anthropic.is_some() {
            keys.push("Anthropic");
        }
        if self.openai.is_some() {
            keys.push("OpenAI");
        }
        if self.github.is_some() {
            keys.push("GitHub");
        }

        if keys.is_empty() {
            "None".to_owned()
        } else {
            keys.join(", ")
        }
    }
}

fn first_present_by_priority(secrets: &SecretsData) -> Option<ProviderId> {
    PROVIDER_PRIORITY
        .into_iter()
        .find(|p| secrets.key_for(p).is_some())
        .map(ProviderId::new)
}

/// An explicit, key-backed `--default-provider` flag wins; otherwise the first
/// present key by [`PROVIDER_PRIORITY`]. The flag's absence is never fatal —
/// `validate_secrets` already guarantees at least one key is present.
pub(super) fn resolve_primary(
    args: &SetupArgs,
    secrets: &SecretsData,
) -> Result<Option<ProviderId>> {
    let Some(name) = args.default_provider.as_deref().map(str::trim) else {
        return Ok(first_present_by_priority(secrets));
    };
    if !STANDARD_PROVIDERS.contains(&name) {
        bail!("--default-provider must be one of: gemini, anthropic, openai (got '{name}')");
    }
    if secrets.key_for(name).is_none() {
        bail!(
            "--default-provider '{name}' has no API key; pass --{name}-key or drop \
             --default-provider"
        );
    }
    Ok(Some(ProviderId::new(name)))
}
