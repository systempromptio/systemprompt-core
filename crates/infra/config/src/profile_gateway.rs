//! Gateway-section post-parse helpers for the profile loader.
//!
//! Catalog resolution itself lives in
//! [`systemprompt_models::profile::GatewayConfigSpec::resolve`]; this
//! module only owns the in-memory route-id backfill applied to the parsed
//! spec before resolution.

use std::path::Path;

use systemprompt_models::profile::{GatewayConfigSpec, ProfileError, ProfileResult};

/// Ids are deterministic, so this in-memory backfill is reapplied identically
/// on every load rather than persisted to disk.
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

/// Resolve `!include <path>` prompt bodies in system-prompt override rules.
///
/// Each file is read relative to the profile directory; a missing file is a
/// hard load error (fail-closed), consistent with the services loader. Like
/// route-id backfill this never writes back to disk.
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
