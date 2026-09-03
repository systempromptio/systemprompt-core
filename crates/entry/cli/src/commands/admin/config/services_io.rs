//! Shared load/save for the services files the `admin config catalog` and
//! `admin config gateway` setters edit.
//!
//! The provider catalog and gateway routes are services-tree files
//! (`ai/providers.yaml`, `ai/gateway.yaml`), not profile sections. Each setter
//! loads its one file typed, mutates it, validates the result against the
//! *merged* services config the process booted with — so a route naming a
//! provider declared in another include still validates — and writes the file
//! back. A file created by an edit is appended to the root `includes:` so the
//! next boot actually loads it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use systemprompt_loader::ServicesBootstrap;
use systemprompt_models::services::{GatewayState, ProviderRegistry, ServicesConfig};

use super::config_section::{ConfigSection, GATEWAY_INCLUDE_RELATIVE, PROVIDERS_INCLUDE_RELATIVE};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvidersFile {
    #[serde(default)]
    pub providers: ProviderRegistry,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayFile {
    #[serde(default)]
    pub gateway: Option<GatewayState>,
}

#[derive(Debug)]
pub struct ServicesFile<T> {
    pub path: PathBuf,
    pub existed: bool,
    pub content: T,
}

pub(super) fn load_providers_file() -> Result<ServicesFile<ProvidersFile>> {
    load_file(ConfigSection::Providers.file_path()?)
}

pub(super) fn load_gateway_file() -> Result<ServicesFile<GatewayFile>> {
    load_file(ConfigSection::Gateway.file_path()?)
}

fn load_file<T: Default + for<'de> Deserialize<'de>>(path: PathBuf) -> Result<ServicesFile<T>> {
    if !path.exists() {
        return Ok(ServicesFile {
            path,
            existed: false,
            content: T::default(),
        });
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let content: T = serde_yaml::from_str(&raw)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(ServicesFile {
        path,
        existed: true,
        content,
    })
}

pub(super) fn save_file<T: Serialize>(file: &ServicesFile<T>, relative: &str) -> Result<()> {
    if let Some(parent) = file.path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let body = serde_yaml::to_string(&file.content).context("Failed to serialize services file")?;
    std::fs::write(&file.path, body)
        .with_context(|| format!("Failed to write {}", file.path.display()))?;
    if !file.existed {
        ensure_included(relative)?;
    }
    Ok(())
}

pub(super) fn booted_services() -> Result<&'static ServicesConfig> {
    ServicesBootstrap::get().context("services config is not loaded")
}

// Why: the registry is validated as the loader will see it — every include's
// providers plus this file's edited list — so a name that collides with
// another include fails here, at the edit, rather than at the next boot.
pub(super) fn merged_registry_after_edit(
    before: &ProviderRegistry,
    after: &ProviderRegistry,
) -> Result<ProviderRegistry> {
    let booted = booted_services()?;
    let mut merged = ProviderRegistry {
        providers: booted
            .providers
            .providers
            .iter()
            .filter(|p| before.find_provider(p.name.as_str()).is_none())
            .cloned()
            .collect(),
    };
    for provider in &after.providers {
        if merged.find_provider(provider.name.as_str()).is_some() {
            anyhow::bail!(
                "provider '{}' is already declared by another services include",
                provider.name.as_str()
            );
        }
        merged.providers.push(provider.clone());
    }
    merged
        .validate()
        .context("provider registry is invalid after edit; refusing to write")?;
    Ok(merged)
}

fn ensure_included(relative: &str) -> Result<()> {
    let root = ConfigSection::Services.file_path()?;
    append_include(&root, relative)
}

// Why: the root aggregator is operator-authored and commented, so the include
// is spliced in as text rather than round-tripped through a YAML value that
// would drop every comment.
pub fn append_include(root: &Path, relative: &str) -> Result<()> {
    let existing = std::fs::read_to_string(root).unwrap_or_default();
    let already = existing.lines().any(|line| {
        let item = line.trim_start().strip_prefix("- ").map(str::trim);
        item.is_some_and(|value| value.trim_matches(['"', '\'']) == relative)
    });
    if already {
        return Ok(());
    }
    let entry = format!("  - {relative}\n");
    // Why: a key that ends the file carries no newline of its own, so the entry
    // would splice onto the same line and `includes:` would parse as a scalar —
    // the include is then silently never loaded.
    let splice = |line_end: Option<usize>| {
        line_end.map_or_else(
            || format!("{existing}\n{entry}"),
            |end| format!("{}{entry}{}", &existing[..end], &existing[end..]),
        )
    };
    let updated = match existing.find("\nincludes:") {
        Some(idx) => {
            let insert_at = idx + "\nincludes:".len();
            splice(existing[insert_at..].find('\n').map(|n| insert_at + n + 1))
        },
        None if existing.starts_with("includes:") => splice(existing.find('\n').map(|n| n + 1)),
        None => {
            let sep = if existing.is_empty() || existing.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            format!("{existing}{sep}includes:\n{entry}")
        },
    };
    std::fs::write(root, updated).with_context(|| format!("Failed to write {}", root.display()))
}

pub(super) const fn providers_relative() -> &'static str {
    PROVIDERS_INCLUDE_RELATIVE
}

pub(super) const fn gateway_relative() -> &'static str {
    GATEWAY_INCLUDE_RELATIVE
}
