//! Typed inputs and outputs for [`super::DeployOrchestrator`].
//!
//! [`DeploySecretsSource`] decides what a deploy is allowed to push. With a
//! Vault profile the container fetches its own secrets at boot, so the deploy
//! pushes only the credentials Vault itself needs to answer — never the
//! contents of `secrets.json`, and never the JWT signing key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_cloud::CloudCredentials;
use systemprompt_identifiers::TenantId;
use systemprompt_models::env::contains_placeholder;
use systemprompt_models::profile::{SecretsConfig, SecretsSource, VaultAuth, VaultSecretsConfig};

pub const VAULT_ADDR_ENV: &str = "VAULT_ADDR";
pub const VAULT_NAMESPACE_ENV: &str = "VAULT_NAMESPACE";

#[derive(Debug)]
pub struct DeployRequest {
    pub tenant_id: TenantId,
    pub tenant_name: String,
    pub profile_name: String,
    pub project_root: PathBuf,
    pub credentials: CloudCredentials,
    pub secrets: DeploySecretsSource,
    pub signing_key_path: PathBuf,
    pub options: DeployOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeploySecretsSource {
    EnvFromFile { path: PathBuf },
    Vault { bootstrap_env: Vec<String> },
}

impl DeploySecretsSource {
    #[must_use]
    pub fn from_profile(secrets: Option<&SecretsConfig>, secrets_path: PathBuf) -> Self {
        match secrets {
            Some(config) if matches!(config.source, SecretsSource::Vault) => {
                let bootstrap_env = config
                    .vault
                    .as_ref()
                    .map(bootstrap_env_names)
                    .unwrap_or_default();
                Self::Vault { bootstrap_env }
            },
            Some(_) | None => Self::EnvFromFile { path: secrets_path },
        }
    }
}

#[must_use]
pub fn bootstrap_env_names(vault: &VaultSecretsConfig) -> Vec<String> {
    let mut names = match &vault.auth {
        VaultAuth::Token {
            token_env,
            token_file,
        } => {
            if token_file.is_some() {
                Vec::new()
            } else {
                vec![token_env.clone()]
            }
        },
        VaultAuth::AppRole {
            role_id_env,
            secret_id_env,
            ..
        } => vec![role_id_env.clone(), secret_id_env.clone()],
        VaultAuth::Kubernetes { .. } => Vec::new(),
    };

    if contains_placeholder(&vault.address) {
        names.push(placeholder_var(&vault.address, VAULT_ADDR_ENV));
    }
    if let Some(namespace) = vault.namespace.as_deref()
        && contains_placeholder(namespace)
    {
        names.push(placeholder_var(namespace, VAULT_NAMESPACE_ENV));
    }

    names.sort();
    names.dedup();
    names
}

fn placeholder_var(value: &str, fallback: &str) -> String {
    value
        .split_once("${")
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(name, _)| name.split(":-").next().unwrap_or(name).to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

#[derive(Debug, Clone, Copy)]
pub struct DeployOptions {
    pub skip_push: bool,
}

#[derive(Debug)]
pub struct DeployReport {
    pub image: String,
    pub status: String,
    pub app_url: Option<String>,
}
