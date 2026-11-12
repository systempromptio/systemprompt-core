//! The grant-scoped secret environment for a spawned MCP server or agent.
//!
//! The `storage` grant resolves to the secret the profile's storage
//! credentials name (nothing for workload identity). Every other grant is
//! passed through to
//! [`Secrets::child_env`](systemprompt_manifest::secrets::Secrets).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::child_env::{ChildBoot, ChildEnvError, ChildGrants};
use systemprompt_manifest::profile::GcsCredentials;
use systemprompt_models::subprocess::{SecretGrant, SecretGrantError};

use super::SecretsService;

#[derive(Debug, thiserror::Error)]
pub enum ChildSecretsError {
    #[error(transparent)]
    Env(#[from] ChildEnvError),
    #[error("the profile's storage credential secret cannot be granted")]
    StorageSecret(#[source] SecretGrantError),
    #[error("a model credential secret cannot be granted")]
    DerivedSecret(#[source] SecretGrantError),
}

impl SecretsService {
    pub fn child_env(
        &self,
        boot: ChildBoot,
        explicit: &[SecretGrant],
        derived: &[String],
    ) -> Result<Vec<(String, String)>, ChildSecretsError> {
        let mut grants = ChildGrants {
            boot,
            explicit: explicit.to_vec(),
            derived: derived
                .iter()
                .map(|name| SecretGrant::from_secret_name(name))
                .collect::<Result<_, _>>()
                .map_err(ChildSecretsError::DerivedSecret)?,
        };
        if explicit.contains(&SecretGrant::Storage)
            && let Some(GcsCredentials::Secret(name)) = self.profile().storage.credentials.as_ref()
        {
            grants.explicit.push(
                SecretGrant::from_secret_name(name.as_str())
                    .map_err(ChildSecretsError::StorageSecret)?,
            );
        }
        Ok(self.get().child_env(&grants)?)
    }
}
