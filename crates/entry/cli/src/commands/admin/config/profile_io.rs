//! Shared profile load/save for the `admin config` setter sub-trees.
//!
//! Every setter follows the same shape: deserialize the on-disk profile,
//! mutate a typed field, write it back. [`save_profile`] revalidates before
//! writing so a config edit can never persist a profile the loader would reject
//! at boot — drift surfaces at the edit, not at the next service start.

use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_models::Profile;

pub(super) fn load_profile(path: &str) -> Result<Profile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read profile: {}", path))?;
    serde_yaml::from_str(&content).with_context(|| format!("Failed to parse profile: {}", path))
}

pub(super) fn save_profile(profile: &Profile, path: &str) -> Result<()> {
    profile
        .validate()
        .context("profile is invalid after edit; refusing to write")?;
    let content = serde_yaml::to_string(profile).context("Failed to serialize profile")?;
    std::fs::write(path, content).with_context(|| format!("Failed to write profile: {}", path))?;
    Ok(())
}

pub(super) fn profile_dir(path: &str) -> &Path {
    Path::new(path).parent().unwrap_or_else(|| Path::new("."))
}
