//! Gateway-section post-parse helpers for the profile loader.
//!
//! Resolution itself lives in
//! [`systemprompt_models::profile::GatewayConfigSpec::resolve`]; this module
//! owns the in-memory fix-ups applied to the parsed spec beforehand: route-id
//! backfill and `!include` prompt resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::profile::{GatewayConfigSpec, ProfileError, ProfileResult};

pub fn backfill_route_ids(spec: &mut GatewayConfigSpec) -> bool {
    let mut mutated = false;
    for route in &mut spec.routes {
        if route.id.as_str().trim().is_empty() {
            route.ensure_id();
            mutated = true;
        }
    }
    mutated
}

pub fn resolve_override_prompt_includes(
    base_dir: &Path,
    spec: &mut GatewayConfigSpec,
) -> ProfileResult<()> {
    for rule in &mut spec.system_prompt_overrides {
        let Some(include_path) = rule
            .prompt
            .as_deref()
            .and_then(|p| p.strip_prefix("!include "))
        else {
            continue;
        };
        let full_path = base_dir.join(include_path.trim());
        let resolved =
            std::fs::read_to_string(&full_path).map_err(|source| ProfileError::ReadFile {
                path: full_path.clone(),
                source,
            })?;
        tracing::debug!(path = %full_path.display(), "resolved system_prompt override include");
        rule.prompt = Some(resolved);
    }
    Ok(())
}
