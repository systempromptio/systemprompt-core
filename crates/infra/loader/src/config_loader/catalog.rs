//! The model catalog, read by convention from `services/ai/`.
//!
//! The catalog is the five files of [`CATALOG_FILES`] beside the services
//! root's `config/` directory, read in that order and merged with duplicate
//! ids refused. Nothing else may declare a catalog section: a root or include
//! file carrying `credentials:`, `providers:`, `models:`, `deployments:` or
//! `gateway:` is refused, so each fact has one home. Every file the loader
//! reads (catalog, root and include) is first checked by
//! [`detect_legacy_shape`]: a pre-0.73 tree fails with the
//! `systemprompt admin config migrate` hint and is never half-loaded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_manifest::services::catalog::{
    CATALOG_FILES, CatalogDocument, detect_legacy_shape,
};
use systemprompt_manifest::services::{GatewayState, ServicesConfig};

use crate::error::{ConfigLoadError, ConfigLoadResult};

const CATALOG_DIR: &str = "ai";

#[must_use]
pub fn catalog_dir(config_dir: &Path) -> PathBuf {
    config_dir.parent().unwrap_or(config_dir).join(CATALOG_DIR)
}

pub(super) fn refuse_legacy(path: &Path, content: &str) -> ConfigLoadResult<()> {
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(content) else {
        return Ok(());
    };
    detect_legacy_shape(&value).map_or(Ok(()), |what| {
        Err(ConfigLoadError::LegacyCatalog {
            path: path.to_path_buf(),
            what,
        })
    })
}

pub(super) fn refuse_catalog_sections(
    path: &Path,
    config: &ServicesConfig,
) -> ConfigLoadResult<()> {
    let section = [
        ("credentials", config.credentials.is_empty()),
        ("providers", config.providers.is_empty()),
        ("models", config.models.is_empty()),
        ("deployments", config.deployments.is_empty()),
        ("gateway", config.gateway.is_none()),
    ]
    .into_iter()
    .find_map(|(name, empty)| (!empty).then_some(name));
    section.map_or(Ok(()), |section| {
        Err(ConfigLoadError::CatalogOutsideConvention {
            path: path.to_path_buf(),
            section,
        })
    })
}

pub fn read_catalog_dir(dir: &Path) -> ConfigLoadResult<CatalogDocument> {
    let mut merged = CatalogDocument::default();
    for name in CATALOG_FILES {
        let path = dir.join(name);
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|source| ConfigLoadError::Io {
            path: path.clone(),
            source,
        })?;
        refuse_legacy(&path, &text)?;
        let document = CatalogDocument::from_yaml_str(&path.display().to_string(), &text)?;
        merged.merge(document)?;
    }
    if let Some(spec) = merged.gateway.as_mut() {
        super::gateway::resolve_override_prompt_includes(dir, spec)?;
    }
    Ok(merged)
}

pub(super) fn install_catalog(config: &mut ServicesConfig, document: CatalogDocument) {
    config.credentials = document.credentials;
    config.providers = document.providers;
    config.models = document.models;
    config.deployments = document.deployments;
    config.gateway = document.gateway.map(GatewayState::Spec);
}
