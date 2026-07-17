//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_cloud::{ProfilePath, SessionKey, SessionStore};
use systemprompt_loader::ProfileLoader;
use systemprompt_models::Profile;

use super::types::SwitchOutput;
use crate::paths::ResolvedPaths;
use crate::shared::CommandOutput;

pub(super) fn execute(profile_name: &str) -> Result<CommandOutput> {
    let paths = ResolvedPaths::discover();
    let profiles_dir = paths.profiles_dir();

    let target_dir = profiles_dir.join(profile_name);
    let profile_config_path = ProfilePath::Config.resolve(&target_dir);

    if !profile_config_path.exists() {
        anyhow::bail!(
            "Profile '{}' not found.\n\nAvailable profiles can be listed with: systemprompt admin \
             session list",
            profile_name
        );
    }

    let new_profile = load_profile(&profile_config_path)?;
    let new_tenant_id = new_profile.cloud.as_ref().and_then(|c| c.tenant_id.clone());
    let session_key = SessionKey::from_tenant_id(new_tenant_id.as_ref());

    let sessions_dir = paths.sessions_dir();
    let mut store = SessionStore::load_or_create(&sessions_dir)?;

    let previous_profile = store.active_profile_name.clone();

    store.set_active_with_profile_path(&session_key, profile_name, profile_config_path);
    store.save(&sessions_dir)?;

    let message = if store.get_valid_session(&session_key).is_some() {
        format!("Switched to profile '{}'", profile_name)
    } else {
        format!(
            "Switched to profile '{}'. No active session — run 'systemprompt admin session login' \
             to authenticate.",
            profile_name
        )
    };

    let output = SwitchOutput {
        previous_profile,
        new_profile: profile_name.to_owned(),
        session_key: session_key.as_storage_key(),
        tenant: new_tenant_id.as_ref().map(|t| t.as_str().to_owned()),
        message,
    };

    Ok(CommandOutput::card_value("Switch Profile", &output))
}

fn load_profile(path: &std::path::Path) -> Result<Profile> {
    ProfileLoader::load_from_path(path).context("Failed to load profile")
}
