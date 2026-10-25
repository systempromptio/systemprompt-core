//! Shared load/save for the catalog files the `admin config` catalog and
//! gateway setters edit.
//!
//! The model catalog and the gateway are services-tree files the loader reads
//! by convention from `services/ai/` (`credentials.yaml`, `providers.yaml`,
//! `models.yaml`, `deployments.yaml`, `gateway.yaml`), never profile sections
//! and never `includes:` entries. A setter reads the catalog document, applies
//! its edit through `ProviderCatalogService` (which refuses an edit whose
//! result does not resolve), writes the files back and re-loads the whole
//! services tree through the real loader; if that load fails the previous
//! files are restored byte for byte and the edit is refused, so a setter never
//! leaves a tree the server would refuse to boot from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use systemprompt_config::{LoadedProfile, read_catalog, write_catalog, write_private_atomic};
use systemprompt_loader::ConfigLoader;
use systemprompt_loader::config_loader::catalog::catalog_dir;
use systemprompt_manifest::Profile;
use systemprompt_manifest::services::catalog::{CATALOG_FILES, CatalogDocument};
use systemprompt_manifest::services::{GatewayConfigSpec, ServicesConfig};

use super::config_section::ConfigSection;

/// The catalog directory and the root services file it belongs to.
#[derive(Debug, Clone)]
pub struct CatalogLocation {
    pub root: PathBuf,
    pub dir: PathBuf,
}

impl CatalogLocation {
    #[must_use]
    pub fn active(loaded: &LoadedProfile) -> Self {
        Self::for_root(ConfigSection::Services.file_path(loaded))
    }

    #[must_use]
    pub fn for_root(root: PathBuf) -> Self {
        let config_dir = root.parent().map(Path::to_path_buf).unwrap_or_default();
        Self {
            dir: catalog_dir(&config_dir),
            root,
        }
    }

    pub fn read(&self) -> Result<CatalogDocument> {
        read_catalog(&self.dir).with_context(|| format!("Failed to read {}", self.dir.display()))
    }
}

pub fn write_gateway(dir: &Path, gateway: Option<&GatewayConfigSpec>) -> Result<()> {
    let path = dir.join("gateway.yaml");
    let Some(gateway) = gateway else {
        return Ok(());
    };
    let section: BTreeMap<&str, &GatewayConfigSpec> =
        std::iter::once(("gateway", gateway)).collect();
    let yaml = serde_yaml::to_string(&section).context("Failed to serialize gateway.yaml")?;
    write_private_atomic(&path, yaml.as_bytes())
        .with_context(|| format!("Failed to write {}", path.display()))
}

fn write_document(location: &CatalogLocation, document: &CatalogDocument) -> Result<()> {
    std::fs::create_dir_all(&location.dir)
        .with_context(|| format!("Failed to create {}", location.dir.display()))?;
    write_catalog(&location.dir, document)
        .with_context(|| format!("Failed to write {}", location.dir.display()))?;
    write_gateway(&location.dir, document.gateway.as_ref())
}

fn snapshot(location: &CatalogLocation) -> Result<Vec<(PathBuf, Option<Vec<u8>>)>> {
    CATALOG_FILES
        .iter()
        .map(|name| {
            let path = location.dir.join(name);
            match std::fs::read(&path) {
                Ok(bytes) => Ok((path, Some(bytes))),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((path, None)),
                Err(error) => {
                    Err(error).with_context(|| format!("Failed to read {}", path.display()))
                },
            }
        })
        .collect()
}

fn restore(files: &[(PathBuf, Option<Vec<u8>>)]) -> Result<()> {
    for (path, bytes) in files {
        match bytes {
            Some(bytes) => write_private_atomic(path, bytes)
                .with_context(|| format!("Failed to restore {}", path.display()))?,
            None if path.exists() => std::fs::remove_file(path)
                .with_context(|| format!("Failed to remove {}", path.display()))?,
            None => {},
        }
    }
    Ok(())
}

pub fn commit_catalog(
    location: &CatalogLocation,
    after: &CatalogDocument,
    profile: &Profile,
) -> Result<ServicesConfig> {
    let previous = snapshot(location)?;
    write_document(location, after)?;
    match ConfigLoader::load_from_path(&location.root, Some(profile)) {
        Ok(services) => Ok(services),
        Err(error) => {
            restore(&previous)?;
            Err(anyhow::anyhow!(
                "the services tree no longer loads after this edit, so it was not saved: {error}"
            ))
        },
    }
}
