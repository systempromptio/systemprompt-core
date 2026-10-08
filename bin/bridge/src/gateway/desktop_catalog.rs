//! Gateway-derived Desktop model policy shared by install and profile repair.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
        models: super::model_view::with_context_variants(&profile.models, &profile.model_limits),
    };
    let bytes = serde_json::to_vec(&catalog).map_err(std::io::Error::other)?;
    crate::fsutil::atomic_write_0600(&dir.join("desktop-models.json"), &bytes)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) fn models() -> Result<Option<String>, CatalogError> {
    let Some(dir) = crate::config::paths::bridge_metadata_dir() else {
        return Ok(None);
    };
    let path = dir.join("desktop-models.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(CatalogError::Io { path, source });
        },
    };
    let catalog: Catalog =
        serde_json::from_slice(&bytes).map_err(|source| CatalogError::Json { path, source })?;
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
        .map_err(CatalogError::Encode)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("read desktop model catalog {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("decode desktop model catalog {path}: {source}")]
    Json {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
    #[error(transparent)]
    Config(#[from] crate::config::ConfigReadError),
    #[error("encode desktop inference models: {0}")]
    Encode(#[source] serde_json::Error),
}
