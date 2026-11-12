//! The secret half of a spawned MCP server's or agent's environment.
//!
//! [`Secrets::child_env`] emits the database URLs and exactly the secrets its
//! manifest grants:
//! explicit grants (a granted secret that is not configured is an error) and
//! derived grants (the agent's model credentials, emitted when configured).
//! No other provider key or custom secret reaches the child, and no child
//! receives the OAuth pepper, manifest seed, signing keys or licence secret.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_models::subprocess::SecretGrant;

use crate::paths::constants::env_vars::CUSTOM_SECRETS;
use crate::secrets::Secrets;

pub const CHILD_DATABASE_SECRETS: &[&str] = &[
    "DATABASE_URL",
    "DATABASE_WRITE_URL",
    "DATABASE_SESSION_URL",
    "EXTERNAL_DATABASE_URL",
    "INTERNAL_DATABASE_URL",
];

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

    #[must_use]
    pub fn is_secret_env_name(&self, name: &str) -> bool {
        systemprompt_models::subprocess::is_api_held_env_name(name)
            || self.custom.keys().any(|key| key.eq_ignore_ascii_case(name))
    }
}
