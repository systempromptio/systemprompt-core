use std::path::Path;
use systemprompt_models::profile::{Profile, ProfileError, ProfileResult};

use crate::profile_gateway;

pub fn load_profile_with_catalog(path: &Path) -> ProfileResult<Profile> {
    let content = std::fs::read_to_string(path).map_err(|source| ProfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let mut profile = Profile::from_yaml(&content, path)?;
    if let Some(gateway) = profile.gateway.as_mut() {
        let profile_dir = path.parent().unwrap_or_else(|| Path::new("."));
        profile_gateway::resolve_catalog(gateway, profile_dir)?;
    }
    Ok(profile)
}
