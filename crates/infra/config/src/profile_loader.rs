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

    // The registry is the authority for upstream connectivity; validate it
    // before any layer that references it.
    profile.providers.validate()?;

    let Some(state) = profile.gateway.take() else {
        return Ok(profile);
    };

    let mut spec = state.into_spec();

    // Route ids are synthesized deterministically from (pattern, provider), so
    // this in-memory backfill yields identical ids on every load. Loading never
    // writes back to disk — that avoids both the concurrent-invocation write
    // race and the risk of baking interpolated `${VAR}` values into the source.
    profile_gateway::backfill_route_ids(&mut spec);

    let resolved = spec.resolve();
    resolved.validate(&profile.providers)?;
    profile.gateway = Some(GatewayState::Resolved(resolved));
    Ok(profile)
}
