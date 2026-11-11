//! The secret half of a spawned MCP server's or agent's environment.
//!
//! [`Secrets::child_env`] emits the database URLs, the [`CHILD_BOOT_SECRETS`]
//! a child cannot start without, and exactly the secrets its manifest grants:
//! explicit grants (a granted secret that is not configured is an error) and
//! derived grants (the agent's model credentials, emitted when configured).
//! No other provider key or custom secret reaches the child.
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

// Why: a child still boots a full AppContext. SecretsService::load needs the
// pepper and validates the seed, master key and (when licensing is on) the
// licence secret; SigningKeys::resolve needs the signing PEMs and the seed.
// These leave the child environment only when children boot a context
// without signing keys or the pepper.
pub const CHILD_BOOT_SECRETS: &[&str] = &[
    "OAUTH_AT_REST_PEPPER",
    "MANIFEST_SIGNING_SECRET_SEED",
    "SIGNING_KEY_PEM",
    "SIGNING_KEY_STAGED_PEM",
    "SIGNING_KEY_PREVIOUS_KEYS",
    "ENCRYPTION_MASTER_KEY",
    "LICENCE_SIGNING_SECRET",
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

        for name in CHILD_DATABASE_SECRETS.iter().chain(CHILD_BOOT_SECRETS) {
            if let Some(value) = self.boot_value(name).filter(|v| !v.is_empty()) {
                env.push(((*name).to_owned(), value.clone()));
                if *name == "ENCRYPTION_MASTER_KEY" {
                    custom_names.insert((*name).to_owned());
                }
            }
        }

        let required = grants.explicit.iter().map(|grant| (grant, true));
        let optional = grants.derived.iter().map(|grant| (grant, false));
        for (grant, is_required) in required.chain(optional) {
            if matches!(
                grant,
                SecretGrant::EncryptionMasterKey | SecretGrant::Storage
            ) {
                continue;
            }
            let value = match grant {
                SecretGrant::Custom(name) => self.custom_secret(name),
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
            if matches!(grant, SecretGrant::Custom(_)) {
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

    fn boot_value(&self, name: &str) -> Option<&String> {
        match name {
            "MANIFEST_SIGNING_SECRET_SEED" => self.manifest_signing_secret_seed.as_ref(),
            "ENCRYPTION_MASTER_KEY" => self.custom_secret("encryption_master_key"),
            "LICENCE_SIGNING_SECRET" => self.custom_secret("LICENCE_SIGNING_SECRET"),
            other => self.get(other),
        }
    }
}
