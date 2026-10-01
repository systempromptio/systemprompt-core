//! Free functions for ad-hoc secrets I/O outside of the global
//! bootstrap singleton.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::secrets::Secrets;

use super::SecretsBootstrapError;
use crate::error::ConfigResult;

pub fn load_secrets_from_path(secrets_path: &Path) -> ConfigResult<Secrets> {
    if !secrets_path.exists() {
        return Err(SecretsBootstrapError::FileNotFound {
            path: secrets_path.display().to_string(),
        }
        .into());
    }
    let content = std::fs::read_to_string(secrets_path)?;
    Secrets::parse(&content).map_err(|e| SecretsBootstrapError::InvalidSecretsFile(e).into())
}
