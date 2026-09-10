//! Secrets document read from a path relative to the active profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::profile::resolve_with_home;
use systemprompt_models::secrets::Secrets;

use crate::bootstrap::profile::ProfileBootstrap;
use crate::bootstrap::secrets::SecretsBootstrapError;
use crate::error::{ConfigError, ConfigResult};

pub(in crate::bootstrap::secrets) fn resolve_and_load_file(
    path_str: &str,
) -> ConfigResult<Secrets> {
    let profile_path =
        ProfileBootstrap::get_path().map_err(|_e| SecretsBootstrapError::ProfileNotInitialized)?;

    let profile_dir = Path::new(profile_path)
        .parent()
        .ok_or_else(|| ConfigError::other("Invalid profile path - no parent directory"))?;

    let resolved_path = resolve_with_home(profile_dir, path_str);
    load_from_file(&resolved_path)
}

fn load_from_file(path: &Path) -> ConfigResult<Secrets> {
    if !path.exists() {
        return Err(SecretsBootstrapError::FileNotFound {
            path: path.display().to_string(),
        }
        .into());
    }

    let content = std::fs::read_to_string(path)?;

    let secrets =
        Secrets::parse(&content).map_err(|e| SecretsBootstrapError::InvalidSecretsFile {
            message: e.to_string(),
        })?;

    tracing::debug!(path = %path.display(), "loaded secrets");

    Ok(secrets)
}
