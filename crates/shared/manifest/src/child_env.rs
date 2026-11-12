//! The secret half of a spawned MCP server's or agent's environment.
//!
//! [`Secrets::child_env`] emits the database URLs and exactly the secrets its
//! manifest grants:
//! explicit grants (a granted secret that is not configured is an error) and
//! derived grants (the agent's model credentials, emitted when configured).
//! No other provider key or custom secret reaches the child, and no child
//! receives the OAuth pepper, manifest seed, signing keys or licence secret.
//!
//! [`refused_env_var`] guards the `env_vars` pass-through: it refuses every
//! API-held name, every environment name the profile's secrets source reads
//! to authenticate (a renamed Vault `token_env`, the AppRole id pair), and,
//! for an `external` source whose provider reads names core cannot know,
//! every entry. [`Secrets::refused_env_var`] adds the custom secret names the
//! loaded document carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_models::subprocess::SecretGrant;

use crate::paths::constants::env_vars::CUSTOM_SECRETS;
use crate::profile::{SecretsConfig, SecretsSource, VaultAuth};
use crate::secrets::Secrets;

pub const CHILD_DATABASE_SECRETS: &[&str] = &[
    "DATABASE_URL",
    "DATABASE_WRITE_URL",
    "DATABASE_SESSION_URL",
    "EXTERNAL_DATABASE_URL",
    "INTERNAL_DATABASE_URL",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvVarRefusal {
    #[error(
        "env_vars names '{name}', which carries a secret or a secrets-source credential; grant \
         it under `secrets:` instead, or drop it if it is held by the API only"
    )]
    CarriesSecret { name: String },
    #[error(
        "env_vars names '{name}', but the profile's `external` secrets source reads environment \
         names core cannot enumerate, so no env_vars pass-through is allowed; grant the value \
         under `secrets:` instead"
    )]
    ExternalSource { name: String },
}

#[must_use]
pub fn source_credential_env_names(config: Option<&SecretsConfig>) -> Option<Vec<&str>> {
    let Some(config) = config else {
        return Some(Vec::new());
    };
    match config.source {
        SecretsSource::File | SecretsSource::Env => Some(Vec::new()),
        SecretsSource::External => None,
        SecretsSource::Vault => Some(match config.vault.as_ref().map(|vault| &vault.auth) {
            Some(VaultAuth::Token { token_env, .. }) => vec![token_env.as_str()],
            Some(VaultAuth::AppRole {
                role_id_env,
                secret_id_env,
                ..
            }) => vec![role_id_env.as_str(), secret_id_env.as_str()],
            Some(VaultAuth::Kubernetes { .. }) | None => Vec::new(),
        }),
    }
}

pub fn refused_env_var(
    config: Option<&SecretsConfig>,
    env_vars: &[String],
) -> Result<(), EnvVarRefusal> {
    let Some(first) = env_vars.first() else {
        return Ok(());
    };
    let Some(credentials) = source_credential_env_names(config) else {
        return Err(EnvVarRefusal::ExternalSource {
            name: first.clone(),
        });
    };
    env_vars
        .iter()
        .find(|name| {
            systemprompt_models::subprocess::is_api_held_env_name(name)
                || credentials
                    .iter()
                    .any(|credential| credential.eq_ignore_ascii_case(name.trim()))
        })
        .map_or(Ok(()), |name| {
            Err(EnvVarRefusal::CarriesSecret { name: name.clone() })
        })
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChildEnvError {
    #[error("secret '{grant}' is granted but not configured in the secrets document")]
    NotConfigured { grant: String },
}

/// The grants one child receives.
#[derive(Debug, Clone, Default)]
pub struct ChildGrants {
    pub explicit: Vec<SecretGrant>,
    pub derived: Vec<SecretGrant>,
}

impl Secrets {
    pub fn child_env(&self, grants: &ChildGrants) -> Result<Vec<(String, String)>, ChildEnvError> {
        let mut env: Vec<(String, String)> = Vec::new();
        let mut custom_names: BTreeSet<String> = BTreeSet::new();

        for name in CHILD_DATABASE_SECRETS {
            if let Some(value) = self.get(name).filter(|v| !v.is_empty()) {
                env.push(((*name).to_owned(), value.clone()));
            }
        }

        let required = grants.explicit.iter().map(|grant| (grant, true));
        let optional = grants.derived.iter().map(|grant| (grant, false));
        for (grant, is_required) in required.chain(optional) {
            if matches!(grant, SecretGrant::Storage) {
                continue;
            }
            let value = match grant {
                SecretGrant::Custom(name) => self.custom_secret(name),
                SecretGrant::EncryptionMasterKey => self.custom_secret("encryption_master_key"),
                other => self.get(other.secret_key()),
            }
            .filter(|v| !v.is_empty());
            let Some(value) = value else {
                if is_required {
                    return Err(ChildEnvError::NotConfigured {
                        grant: grant.to_string(),
                    });
                }
                continue;
            };
            let env_name = grant.env_name();
            if env.iter().any(|(name, _)| *name == env_name) {
                continue;
            }
            if matches!(
                grant,
                SecretGrant::Custom(_) | SecretGrant::EncryptionMasterKey
            ) {
                custom_names.insert(env_name.clone());
            }
            env.push((env_name, value.clone()));
        }

        if !custom_names.is_empty() {
            env.push((
                CUSTOM_SECRETS.to_owned(),
                custom_names.into_iter().collect::<Vec<_>>().join(","),
            ));
        }
        Ok(env)
    }

    pub fn refused_env_var(
        &self,
        config: Option<&SecretsConfig>,
        env_vars: &[String],
    ) -> Result<(), EnvVarRefusal> {
        refused_env_var(config, env_vars)?;
        env_vars
            .iter()
            .find(|name| {
                self.custom
                    .keys()
                    .any(|key| key.eq_ignore_ascii_case(name.trim()))
            })
            .map_or(Ok(()), |name| {
                Err(EnvVarRefusal::CarriesSecret { name: name.clone() })
            })
    }
}
