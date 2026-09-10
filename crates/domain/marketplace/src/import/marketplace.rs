//! `marketplace.json` (+ optional sidecar) → `marketplaces/<id>/config.yaml`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::marketplace::{MarketplaceConfig, MarketplaceConfigFile};
use systemprompt_models::services::plugin::{ComponentSource, PluginAuthor, PluginComponentRef};

use crate::error::MarketplaceError;

use super::anthropic::MarketplaceJson;
use super::sidecar::MarketplaceSidecar;
use super::writer::Sink;

pub(super) const DEFAULT_LICENSE: &str = "proprietary";

pub(super) const DEFAULT_VERSION: &str = "0.1.0";

pub(super) fn import_marketplace(
    manifest: &MarketplaceJson,
    sidecar: &MarketplaceSidecar,
    manifest_path: &Path,
    sink: &Sink,
) -> Result<MarketplaceId, MarketplaceError> {
    let id = MarketplaceId::new(manifest.name.trim());

    let version = if manifest.metadata.version.trim().is_empty() {
        DEFAULT_VERSION.to_owned()
    } else {
        manifest.metadata.version.clone()
    };

    let config = MarketplaceConfig {
        id: id.clone(),
        name: sidecar
            .marketplace
            .title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| manifest.name.clone()),
        description: manifest.metadata.description.clone(),
        version,
        enabled: sidecar.marketplace.enabled,
        author: PluginAuthor {
            name: manifest.owner.name.clone(),
            email: manifest.owner.email.clone(),
        },
        keywords: Vec::new(),
        license: DEFAULT_LICENSE.to_owned(),
        visibility: sidecar.marketplace.visibility,
        plugins: PluginComponentRef {
            source: ComponentSource::Explicit,
            filter: None,
            include: manifest.plugins.iter().map(|p| p.name.clone()).collect(),
            exclude: Vec::new(),
        },
        mcp_servers: sidecar.marketplace.mcp_servers.clone(),
        agents: sidecar.marketplace.agents.clone(),
        artifacts: sidecar.marketplace.artifacts.clone(),
        access: sidecar.marketplace.access.clone(),
    };

    config
        .validate(id.as_str())
        .map_err(|e| MarketplaceError::Import {
            path: manifest_path.display().to_string(),
            message: e.to_string(),
        })?;

    let rel = Path::new("marketplaces")
        .join(id.as_str())
        .join("config.yaml");
    sink.write_yaml(
        &rel,
        &MarketplaceConfigFile {
            marketplace: config,
        },
    )?;

    Ok(id)
}
