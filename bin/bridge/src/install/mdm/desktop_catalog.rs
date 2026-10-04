//! Gateway-derived Desktop model policy shared by install and profile repair.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::MdmError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Catalog {
    gateway: String,
    models: Vec<String>,
}

pub(crate) fn remember(
    gateway: &str,
    profile: &crate::gateway::types::BridgeProfile,
) -> std::io::Result<()> {
    let dir = crate::config::paths::bridge_metadata_dir()
        .ok_or_else(|| std::io::Error::other("bridge metadata directory unavailable"))?;
    std::fs::create_dir_all(&dir)?;
    let catalog = Catalog {
        gateway: gateway.trim_end_matches('/').to_owned(),
        models: crate::integration::claude_desktop::reg_profile::with_context_variants(
            &profile.models,
            &profile.model_limits,
        ),
    };
    let bytes = serde_json::to_vec(&catalog).map_err(std::io::Error::other)?;
    crate::fsutil::atomic_write_0600(&dir.join("desktop-models.json"), &bytes)
}

pub(crate) fn models() -> Result<Option<String>, MdmError> {
    let Some(dir) = crate::config::paths::bridge_metadata_dir() else {
        return Ok(None);
    };
    let path = dir.join("desktop-models.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(MdmError::Io {
                action: "read desktop model catalog",
                path,
                source,
            });
        },
    };
    let catalog: Catalog =
        serde_json::from_slice(&bytes).map_err(|source| MdmError::Json { path, source })?;
    let cfg = crate::config::load()?;
    if catalog.gateway
        != crate::config::gateway_url_or_default(&cfg)
            .as_str()
            .trim_end_matches('/')
    {
        return Ok(None);
    }
    serde_json::to_string(&catalog.models)
        .map(Some)
        .map_err(|source| MdmError::ConfigJson {
            key: "inferenceModels",
            source,
        })
}
