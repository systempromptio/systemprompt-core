//! Free functions for ad-hoc secrets I/O outside of the global
//! bootstrap singleton.

use std::path::Path;

use systemprompt_models::profile::SecretsValidationMode;
use systemprompt_models::secrets::Secrets;

use super::{SecretsBootstrapError, log_secrets_issue};
use crate::error::{ConfigError, ConfigResult};

pub fn load_secrets_from_path(secrets_path: &Path) -> ConfigResult<Secrets> {
    if !secrets_path.exists() {
        return Err(SecretsBootstrapError::FileNotFound {
            path: secrets_path.display().to_string(),
        }
        .into());
    }
    let content = std::fs::read_to_string(secrets_path)?;
    Secrets::parse(&content).map_err(|e| {
        SecretsBootstrapError::InvalidSecretsFile {
            message: e.to_string(),
        }
        .into()
    })
}

pub fn handle_load_error(e: ConfigError, mode: SecretsValidationMode) -> ConfigResult<Secrets> {
    log_secrets_issue(&e, mode);
    Err(e)
}
