//! Gateway-section post-parse fix-ups for the services loader.
//!
//! Resolution itself lives in
//! [`systemprompt_models::services::GatewayConfigSpec::resolve`]; this module
//! owns what happens to the parsed spec beforehand — route-id backfill and
//! `!include` prompt resolution — and the final projection to
//! [`GatewayState::Resolved`] once every include has been merged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::services::{GatewayConfigSpec, GatewayState, ServicesConfig};

use crate::error::{ConfigLoadError, ConfigLoadResult};

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
) -> ConfigLoadResult<()> {
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
            std::fs::read_to_string(&full_path).map_err(|source| ConfigLoadError::Io {
                path: full_path.clone(),
                source,
            })?;
        tracing::debug!(path = %full_path.display(), "resolved system_prompt override include");
        rule.prompt = Some(resolved);
    }
    Ok(())
}

// Why: `!include` paths are relative to the file that wrote them, and that file
// is only known while it is being parsed — so each file resolves its own
// gateway prompts before the merge, the same way agent system prompts are.
pub(super) fn resolve_file_gateway_includes(
    file_dir: &Path,
    config: &mut ServicesConfig,
) -> ConfigLoadResult<()> {
    if let Some(spec) = config.gateway.as_mut().and_then(GatewayState::as_spec_mut) {
        resolve_override_prompt_includes(file_dir, spec)?;
    }
    Ok(())
}

// Why: the cache only ever holds a resolved gateway, so no runtime reader can
// observe `GatewayState::Spec` — the loader is the single place the projection
// happens. Validation against the merged registry runs in
// `ServicesConfig::validate`, which the caller invokes right after this.
pub(super) fn project_gateway(config: &mut ServicesConfig) {
    let Some(state) = config.gateway.take() else {
        return;
    };
    let mut spec = state.into_spec();
    backfill_route_ids(&mut spec);
    config.gateway = Some(GatewayState::Resolved(spec.resolve()));
}
