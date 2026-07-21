//! Projects on-disk artifact directories into the signed `ArtifactEntry`
//! records the manifest carries.
//!
//! Each `services/artifacts/<id>/config.yaml` plus its HTML body (default
//! `content.html`) becomes one Cowork library document. Fail-closed: an
//! artifact with empty HTML content or no `mcp_tools` is dropped with a warning
//! rather than shipped inert, mirroring the plugin-bundle drop in
//! [`crate::catalog::plugin_bundles`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::ArtifactEntry;
use systemprompt_models::services::{ARTIFACT_CONFIG_FILENAME, DiskArtifactConfig};

use crate::error::MarketplaceError;

pub fn load_artifacts(services_root: &Path) -> Result<Vec<ArtifactEntry>, MarketplaceError> {
    let artifacts_dir = services_root.join("artifacts");
    if !artifacts_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    let read =
        std::fs::read_dir(&artifacts_dir).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    for entry in read {
        let entry = entry.map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !path.join(ARTIFACT_CONFIG_FILENAME).exists() {
            continue;
        }
        entries.push((dir_name.to_owned(), path));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::with_capacity(entries.len());
    for (_dir_name, artifact_dir) in entries {
        match build_artifact_entry(&artifact_dir) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {},
            Err(e) => {
                tracing::warn!(
                    artifact_dir = %artifact_dir.display(),
                    error = %e,
                    "manifest: failed to build artifact entry; skipping"
                );
            },
        }
    }
    Ok(out)
}

fn build_artifact_entry(artifact_dir: &Path) -> Result<Option<ArtifactEntry>, MarketplaceError> {
    let config_path = artifact_dir.join(ARTIFACT_CONFIG_FILENAME);
    let config_text = std::fs::read_to_string(&config_path)
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    let config: DiskArtifactConfig = serde_yaml::from_str(&config_text)
        .map_err(|e| MarketplaceError::Catalog(format!("parse {}: {e}", config_path.display())))?;

    if !config.enabled {
        return Ok(None);
    }

    let content_path = artifact_dir.join(config.content_file());
    let content = if content_path.exists() {
        std::fs::read_to_string(&content_path)
            .map_err(|e| MarketplaceError::Catalog(e.to_string()))?
    } else {
        String::new()
    };

    if content.trim().is_empty() {
        tracing::warn!(
            artifact_id = %config.id.as_str(),
            "marketplace: artifact has no HTML content; skipping"
        );
        return Ok(None);
    }
    if config.mcp_tools.is_empty() {
        tracing::warn!(
            artifact_id = %config.id.as_str(),
            "marketplace: artifact references no mcp_tools; skipping"
        );
        return Ok(None);
    }

    let sha256 = artifact_digest(
        config.id.as_str(),
        &config.version,
        &content,
        &config.mcp_tools,
    )?;

    Ok(Some(ArtifactEntry {
        id: config.id,
        name: config.name,
        description: config.description,
        version: config.version,
        mcp_tools: config.mcp_tools,
        content,
        starred: config.starred,
        sha256,
    }))
}

/// NUL-delimited so no concatenation of two fields can collide with a
/// different split.
fn artifact_digest(
    id: &str,
    version: &str,
    content: &str,
    mcp_tools: &[String],
) -> Result<Sha256Digest, MarketplaceError> {
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    hasher.update([0u8]);
    hasher.update(version.as_bytes());
    hasher.update([0u8]);
    hasher.update(content.as_bytes());
    hasher.update([0u8]);
    for tool in mcp_tools {
        hasher.update(tool.as_bytes());
        hasher.update([0u8]);
    }
    Sha256Digest::try_new(hex::encode(hasher.finalize()))
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))
}
