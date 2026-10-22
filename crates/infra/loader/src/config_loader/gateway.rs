//! Gateway-section post-parse fix-ups for the services loader.
//!
//! Resolution itself lives in
//! [`systemprompt_manifest::services::GatewayConfigSpec::resolve`]; this module
//! owns `!include` prompt resolution (relative to `services/ai/`, where
//! `gateway.yaml` lives) and the final projection to
//! [`GatewayState::Resolved`]. Every route declares its id, so nothing is
//! backfilled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_manifest::services::{GatewayConfigSpec, GatewayState, ServicesConfig};

use crate::error::{ConfigLoadError, ConfigLoadResult};

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

pub(super) fn project_gateway(config: &mut ServicesConfig) {
    let Some(state) = config.gateway.take() else {
        return;
    };
    config.gateway = Some(GatewayState::Resolved(state.into_spec().resolve()));
}
