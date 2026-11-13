//! Secrets data model and default-provider resolution.
//!
//! [`SecretsData`] holds the generated OAuth at-rest pepper, encryption master
//! key, database URL, and AI-provider keys. [`resolve_primary`] picks the
//! default provider from an explicit flag or the first configured provider by
//! [`PROVIDER_PRIORITY`].
//!
//! Vertex AI is configured either by a service-account key (stored as the
//! `vertex` secret) or by a project alone, which authenticates as the
//! runtime's workload identity and stores nothing; that project is carried
//! here for the catalog seed but is not a secret and is never written to the
//! secrets file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ProviderId;

use super::super::SetupArgs;
use crate::shared::provider_catalog::{VERTEX, VertexSeed};

pub(super) const STANDARD_PROVIDERS: [&str; 4] = ["gemini", "anthropic", "openai", VERTEX];

const PROVIDER_PRIORITY: [&str; 4] = ["anthropic", "openai", "gemini", VERTEX];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsData {
    pub oauth_at_rest_pepper: String,

    pub encryption_master_key: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_signing_secret_seed: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key_pem: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,

    #[serde(skip)]
    pub vertex_project: Option<String>,
}

impl SecretsData {
    pub(crate) const fn has_ai_provider(&self) -> bool {
        self.gemini.is_some()
            || self.anthropic.is_some()
            || self.openai.is_some()
            || self.vertex_seed().is_some()
    }

    #[must_use]
    pub const fn vertex_seed(&self) -> Option<VertexSeed<'_>> {
        match (&self.vertex, &self.vertex_project) {
            (Some(_), _) => Some(VertexSeed::ServiceAccountKey),
            (None, Some(project)) => Some(VertexSeed::WorkloadIdentity {
                project: project.as_str(),
            }),
            (None, None) => None,
        }
    }

    fn is_configured(&self, provider: &str) -> bool {
        match provider {
            "gemini" => self.gemini.is_some(),
            "anthropic" => self.anthropic.is_some(),
            "openai" => self.openai.is_some(),
            VERTEX => self.vertex_seed().is_some(),
            _ => false,
        }
    }

    pub(crate) fn present_providers(&self) -> Vec<&'static str> {
        STANDARD_PROVIDERS
            .into_iter()
            .filter(|p| self.is_configured(p))
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
        match self.vertex_seed() {
            Some(VertexSeed::ServiceAccountKey) => keys.push("Vertex AI (service-account key)"),
            Some(VertexSeed::WorkloadIdentity { .. }) => {
                keys.push("Vertex AI (workload identity)");
            },
            None => {},
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
        .find(|p| secrets.is_configured(p))
        .map(ProviderId::new)
}

pub(super) fn resolve_primary(
    args: &SetupArgs,
    secrets: &SecretsData,
) -> Result<Option<ProviderId>> {
    let Some(name) = args.default_provider.as_deref().map(str::trim) else {
        return Ok(first_present_by_priority(secrets));
    };
    if !STANDARD_PROVIDERS.contains(&name) {
        bail!(
            "--default-provider must be one of: gemini, anthropic, openai, vertex (got '{name}')"
        );
    }
    if !secrets.is_configured(name) {
        let flag = if name == VERTEX {
            "--vertex-project or --vertex-key-file".to_owned()
        } else {
            format!("--{name}-key")
        };
        bail!(
            "--default-provider '{name}' is not configured; pass {flag} or drop --default-provider"
        );
    }
    Ok(Some(ProviderId::new(name)))
}
