//! YAML profile loader with embedded gateway-catalog resolution.

use std::path::Path;

use systemprompt_models::profile::{GatewayState, Profile, ProfileError, ProfileResult};

use crate::profile_gateway;

pub fn load_profile_with_catalog(path: &Path) -> ProfileResult<Profile> {
    let content = std::fs::read_to_string(path).map_err(|source| ProfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let mut profile = Profile::from_yaml(&content, path)?;

    let Some(state) = profile.gateway.take() else {
        return Ok(profile);
    };

    let profile_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut spec = state.into_spec();

    if profile_gateway::backfill_route_ids(&mut spec) {
        profile.gateway = Some(GatewayState::Spec(spec.clone()));
        persist_profile(path, &profile)?;
    }

    let resolved = spec.resolve(profile_dir)?;
    profile.gateway = Some(GatewayState::Resolved(resolved));
    Ok(profile)
}

fn persist_profile(path: &Path, profile: &Profile) -> ProfileResult<()> {
    let yaml = profile.to_yaml()?;
    std::fs::write(path, yaml).map_err(|source| ProfileError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}
