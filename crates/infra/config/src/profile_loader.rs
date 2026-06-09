//! YAML profile loader: parses a profile and projects its gateway section to
//! runtime form, validating the provider registry and gateway references.

use std::path::Path;

use systemprompt_models::profile::{GatewayState, Profile, ProfileError, ProfileResult};

use crate::profile_gateway;

pub fn load_profile_with_catalog(path: &Path) -> ProfileResult<Profile> {
    let content = std::fs::read_to_string(path).map_err(|source| ProfileError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let mut profile = Profile::from_yaml(&content, path)?;

    profile.providers.validate()?;

    let Some(state) = profile.gateway.take() else {
        return Ok(profile);
    };

    let mut spec = state.into_spec();

    profile_gateway::backfill_route_ids(&mut spec);

    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    profile_gateway::resolve_override_prompt_includes(base_dir, &mut spec)?;

    let resolved = spec.resolve();
    resolved.validate(&profile.providers)?;
    profile.gateway = Some(GatewayState::Resolved(resolved));
    Ok(profile)
}
