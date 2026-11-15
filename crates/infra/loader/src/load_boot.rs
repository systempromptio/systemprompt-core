//! Loading the owned [`BootContext`] for a profile.
//!
//! The CLI runner builds one for the invocation's profile; commands that act
//! on a different profile than the one the invocation booted (session
//! resolution, profile inspection) build their own from the profile path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_config::{ConfigError, LoadedProfile, SecretsService, SecretsSources};

use crate::boot::BootContext;

#[derive(Debug, thiserror::Error)]
pub enum BootLoadError {
    #[error("Profile initialization failed from: {}", path.display())]
    Profile {
        path: PathBuf,
        #[source]
        source: ConfigError,
    },
    #[error("Secrets initialization failed")]
    Secrets(#[source] ConfigError),
}

pub async fn load_boot(
    profile: &Arc<LoadedProfile>,
    sources: SecretsSources,
) -> Result<BootContext, BootLoadError> {
    let secrets = SecretsService::load(Arc::clone(profile), sources)
        .await
        .map_err(BootLoadError::Secrets)?;
    Ok(BootContext::new(Arc::clone(profile), secrets))
}

pub async fn load_boot_from_path(
    path: &Path,
    sources: SecretsSources,
) -> Result<BootContext, BootLoadError> {
    let profile = LoadedProfile::from_path(path).map_err(|source| BootLoadError::Profile {
        path: path.to_path_buf(),
        source,
    })?;
    load_boot(&profile, sources).await
}
