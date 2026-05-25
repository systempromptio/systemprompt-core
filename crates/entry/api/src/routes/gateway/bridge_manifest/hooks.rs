//! Hook discovery for the bridge manifest.
//!
//! Scans the services `hooks/` directory, parses each enabled hook config,
//! and builds signed `HookEntry` records with content hashes.

use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_identifiers::HookId;
use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::manifest::HookEntry;
use systemprompt_models::services::DiskHookConfig;
use systemprompt_models::services::hooks::HOOK_CONFIG_FILENAME;

pub(super) fn load_hooks(services_root: &Path) -> anyhow::Result<Vec<HookEntry>> {
    let hooks_dir = services_root.join("hooks");
    if !hooks_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&hooks_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let config_path = path.join(HOOK_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }
        entries.push((dir_name.to_owned(), config_path));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::with_capacity(entries.len());
    for (dir_name, config_path) in entries {
        match build_hook_entry(&dir_name, &config_path) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {},
            Err(e) => {
                tracing::warn!(
                    hook_dir = %dir_name,
                    error = %e,
                    "manifest: failed to build hook entry; skipping"
                );
            },
        }
    }
    Ok(out)
}

fn build_hook_entry(dir_name: &str, config_path: &Path) -> anyhow::Result<Option<HookEntry>> {
    let config_text = std::fs::read_to_string(config_path)?;
    let config: DiskHookConfig = serde_yaml::from_str(&config_text)
        .map_err(|e| anyhow::anyhow!("parse {}: {e}", config_path.display()))?;

    if !config.enabled {
        return Ok(None);
    }

    let id = if config.id.as_str().is_empty() {
        HookId::new(dir_name.replace('-', "_"))
    } else {
        HookId::new(config.id.as_str())
    };
    let name = if config.name.is_empty() {
        dir_name.replace('_', " ")
    } else {
        config.name.clone()
    };

    let mut hasher = Sha256::new();
    hasher.update(config_text.as_bytes());
    let sha256 = Sha256Digest::try_new(hex::encode(hasher.finalize()))?;

    Ok(Some(HookEntry {
        id,
        name,
        description: config.description,
        version: config.version,
        event: config.event,
        matcher: config.matcher,
        command: config.command,
        is_async: config.is_async,
        category: config.category,
        tags: config.tags,
        sha256,
    }))
}
