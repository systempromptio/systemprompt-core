//! Enumerates configured entries independently of enabled manifest projections.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ConfiguredInventoryEntry;
use crate::managed::{ManagedError, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use systemprompt_models::feedback::inventory::InventoryAvailability;
use systemprompt_models::services::ServicesConfig;

// Why: a fetched services composition is served through the loader's atomic
// `current` link, so the root itself may be that one link; it resolves to the
// content-addressed composed directory, and every path beneath it is still
// walked without following links.
pub(crate) fn resolve_services_root(root: &Path) -> Result<PathBuf> {
    if !std::fs::symlink_metadata(root)?.is_symlink() {
        return Ok(root.to_path_buf());
    }
    let resolved = std::fs::canonicalize(root)?;
    if !resolved.is_dir() {
        return Err(invalid("Configured inventory root link does not name a directory"));
    }
    Ok(resolved)
}

pub fn scan_configured_inventory(
    root: &Path,
    services: &ServicesConfig,
) -> Result<Vec<ConfiguredInventoryEntry>> {
    let resolved = resolve_services_root(root)?;
    let root = resolved.as_path();
    let mut entries = Vec::new();
    for (directory, kind) in [
        ("skills", "skill"),
        ("plugins", "plugin"),
        ("marketplaces", "marketplace"),
        ("rules", "rule"),
        ("hooks", "hook"),
        ("artifacts", "artifact"),
    ] {
        scan_catalog_directory(root, directory, kind, &mut entries)?;
    }
    for (key, agent) in &services.agents {
        entries.push(ConfiguredInventoryEntry {
            kind: "agent".to_owned(),
            resource_key: key.clone(),
            relative_root: format!("configured/agents/{key}"),
            availability: if agent.enabled {
                InventoryAvailability::Available
            } else {
                InventoryAvailability::Unavailable
            },
            diagnostic: (!agent.enabled).then(|| "Configured agent is disabled".to_owned()),
        });
    }
    for (key, server) in &services.mcp_servers {
        entries.push(ConfiguredInventoryEntry {
            kind: "mcp".to_owned(),
            resource_key: key.clone(),
            relative_root: format!("configured/mcp/{key}"),
            availability: if server.enabled {
                InventoryAvailability::Available
            } else {
                InventoryAvailability::Unavailable
            },
            diagnostic: (!server.enabled).then(|| "Configured MCP server is disabled".to_owned()),
        });
    }
    if entries.len() > 10_000 {
        return Err(invalid("Inventory exceeds 10000 entries"));
    }
    Ok(entries)
}

fn scan_catalog_directory(
    root: &Path,
    directory: &str,
    kind: &str,
    entries: &mut Vec<ConfiguredInventoryEntry>,
) -> Result<()> {
    let path = root.join(directory);
    let listing = match std::fs::read_dir(&path) {
        Ok(listing) => listing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if std::fs::symlink_metadata(&path)?.is_symlink() {
        return Err(invalid("Configured catalog is a symlink"));
    }
    for entry in listing {
        let entry = entry?;
        if entries.len() >= 10_000 {
            return Err(invalid("Inventory exceeds 10000 configured entries"));
        }
        let name = entry.file_name().into_string().map_err(|name| {
            invalid(&format!(
                "Inventory names must be UTF-8: {}",
                name.to_string_lossy()
            ))
        })?;
        if name.starts_with('.') {
            continue;
        }
        let metadata = entry.file_type()?;
        if !metadata.is_dir()
            && !metadata.is_symlink()
            && !matches!(
                entry.path().extension().and_then(|value| value.to_str()),
                Some("yaml" | "yml" | "md" | "json")
            )
        {
            continue;
        }
        let key = if metadata.is_dir() || metadata.is_symlink() {
            name.clone()
        } else {
            entry
                .path()
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| invalid("Invalid catalog filename"))?
                .to_owned()
        };
        let relative_root = format!("{directory}/{name}");
        let config = if metadata.is_dir() {
            entry.path().join("config.yaml")
        } else {
            entry.path()
        };
        let diagnostic = if metadata.is_symlink() {
            Some("Catalog entry is a symlink".to_owned())
        } else {
            inspect(&config, kind, &key)
                .err()
                .map(|error| error.to_string())
        };
        entries.push(ConfiguredInventoryEntry {
            kind: kind.to_owned(),
            resource_key: key,
            relative_root,
            availability: if diagnostic.is_some() {
                InventoryAvailability::Unavailable
            } else {
                InventoryAvailability::Available
            },
            diagnostic,
        });
    }
    Ok(())
}

fn inspect(path: &Path, kind: &str, key: &str) -> Result<()> {
    if std::fs::symlink_metadata(path)?.is_symlink() {
        return Err(invalid("Catalog configuration is a symlink"));
    }
    if matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("md")
    ) {
        return Ok(());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err(invalid("Catalog configuration exceeds 64 KiB"));
    }
    let config: serde_yaml::Value = serde_yaml::from_slice(&bytes)
        .map_err(|error| invalid(&format!("Catalog configuration is invalid: {error}")))?;
    if config.get("enabled").and_then(serde_yaml::Value::as_bool) == Some(false) {
        return Err(invalid("Configured entry is disabled"));
    }
    if config
        .get("id")
        .and_then(serde_yaml::Value::as_str)
        .is_some_and(|id| !id.is_empty() && id != key)
    {
        return Err(invalid("Configured identity conflicts with its path"));
    }
    if kind == "skill" {
        let parent = path
            .parent()
            .ok_or_else(|| invalid("Missing skill directory"))?;
        let content = config
            .get("file")
            .and_then(serde_yaml::Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("index.md");
        crate::managed::validate_inventory_path(content)?;
        if std::fs::symlink_metadata(parent.join(content))?.is_symlink() {
            return Err(invalid("Skill instruction file is a symlink"));
        }
        if !parent.join(content).is_file() {
            return Err(invalid("Configured skill instruction file is unavailable"));
        }
    }
    Ok(())
}

pub(super) fn invalid(message: &str) -> ManagedError {
    ManagedError::Invalid(message.to_owned())
}
