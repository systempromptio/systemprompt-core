//! Loading the owned [`BootContext`] for a profile.
//!
//! The runner builds one for the invocation's profile; commands that act on a
//! different profile than the one the invocation booted (session resolution,
//! profile inspection) build their own from the profile path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_config::{LoadedProfile, SecretsService, SecretsSources};
use systemprompt_loader::BootContext;

pub fn load_profile(path: &Path) -> Result<Arc<LoadedProfile>> {
    LoadedProfile::from_path(path)
        .with_context(|| format!("Profile initialization failed from: {}", path.display()))
}

pub async fn load_boot(
    profile: &Arc<LoadedProfile>,
    sources: SecretsSources,
) -> Result<BootContext> {
    let secrets = SecretsService::load(Arc::clone(profile), sources)
        .await
        .context("Secrets initialization failed")?;
    Ok(BootContext::new(Arc::clone(profile), secrets))
}

pub async fn load_boot_from_path(path: &Path, sources: SecretsSources) -> Result<BootContext> {
    load_boot(&load_profile(path)?, sources).await
}
