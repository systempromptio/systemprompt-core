//! Reading and writing the catalog files of one `services/ai` directory.
//!
//! [`read_catalog`] merges every present file of
//! [`CATALOG_FILES`](systemprompt_manifest::services::catalog::CATALOG_FILES);
//! [`write_catalog`] writes the four registries back, one section per file,
//! and leaves `gateway.yaml` untouched because no catalog upsert edits it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use systemprompt_manifest::services::catalog::{CATALOG_FILES, CatalogDocument};

use crate::error::{ConfigError, ConfigResult};
use crate::private_file::write_private_atomic;

pub fn read_catalog(dir: &Path) -> ConfigResult<CatalogDocument> {
    let mut merged = CatalogDocument::default();
    for name in CATALOG_FILES {
        let path = dir.join(name);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let document = CatalogDocument::from_yaml_str(&path.display().to_string(), &text)
            .map_err(ConfigError::CatalogDocument)?;
        merged
            .merge(document)
            .map_err(ConfigError::CatalogDocument)?;
    }
    Ok(merged)
}

fn write_section<T: Serialize>(dir: &Path, file: &str, key: &str, value: &T) -> ConfigResult<()> {
    let section: BTreeMap<&str, &T> = std::iter::once((key, value)).collect();
    let yaml = serde_yaml::to_string(&section)?;
    write_private_atomic(&dir.join(file), yaml.as_bytes())?;
    Ok(())
}

pub fn write_catalog(dir: &Path, document: &CatalogDocument) -> ConfigResult<()> {
    write_section(
        dir,
        "credentials.yaml",
        "credentials",
        &document.credentials,
    )?;
    write_section(dir, "providers.yaml", "providers", &document.providers)?;
    write_section(dir, "models.yaml", "models", &document.models)?;
    write_section(
        dir,
        "deployments.yaml",
        "deployments",
        &document.deployments,
    )?;
    Ok(())
}
