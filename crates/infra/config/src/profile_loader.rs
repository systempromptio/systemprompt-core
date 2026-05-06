//! YAML profile loader with embedded gateway-catalog resolution.

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
        let mutated = backfill_route_ids(gateway);
        if mutated {
            persist_profile(path, &profile)?;
        }
    }
    Ok(profile)
}

fn backfill_route_ids(gateway: &mut systemprompt_models::profile::GatewayConfig) -> bool {
    let mut mutated = false;
    for route in &mut gateway.routes {
        if route.id.trim().is_empty() {
            route.ensure_id();
            mutated = true;
        }
    }
    mutated
}

fn persist_profile(path: &Path, profile: &Profile) -> ProfileResult<()> {
    let yaml = profile.to_yaml()?;
    std::fs::write(path, yaml).map_err(|source| ProfileError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}
