//! Profile `secrets:` block naming the secret source and its parameters.
//!
//! `secrets_path` is only meaningful for [`SecretsSource::File`] and
//! [`SecretsSource::Env`] (which falls back to the file when run outside a
//! deployment host), so it is optional and reached through
//! [`SecretsConfig::secrets_path`], which reports the misconfiguration rather
//! than substituting an empty path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::ProfileError;
use super::vault::VaultSecretsConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SecretsSource {
    File,
    Env,
    Vault,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecretsConfig {
    pub source: SecretsSource,

    #[serde(default)]
    pub validation: SecretsValidationMode,

    #[serde(default)]
    pub secrets_path: Option<String>,

    #[serde(default)]
    pub vault: Option<VaultSecretsConfig>,
}

impl SecretsConfig {
    pub fn validate(&self) -> Result<(), ProfileError> {
        match self.source {
            SecretsSource::File | SecretsSource::Env => {
                if matches!(self.source, SecretsSource::File)
                    && self.secrets_path.as_deref().is_none_or(str::is_empty)
                {
                    return Err(ProfileError::SecretsPathRequired {
                        secrets_source: self.source_name(),
                    });
                }
                if self.vault.is_some() {
                    return Err(ProfileError::VaultBlockUnexpected {
                        secrets_source: self.source_name(),
                    });
                }
            },
            SecretsSource::Vault => {
                if self.vault.is_none() {
                    return Err(ProfileError::VaultBlockRequired);
                }
            },
        }
        Ok(())
    }

    pub fn secrets_path(&self) -> Result<&str, ProfileError> {
        self.secrets_path
            .as_deref()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| ProfileError::SecretsPathRequired {
                secrets_source: self.source_name(),
            })
    }

    #[must_use]
    pub const fn source_name(&self) -> &'static str {
        match self.source {
            SecretsSource::File => "file",
            SecretsSource::Env => "env",
            SecretsSource::Vault => "vault",
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum SecretsValidationMode {
    Strict,

    #[default]
    Warn,

    Skip,
}
