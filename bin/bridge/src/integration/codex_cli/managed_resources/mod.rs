//! Codex CLI sync emitter.
//!
//! Codex only loads plugins it resolves *through a marketplace*; a bare plugin
//! folder plus a `[plugins.*].enabled` flag is ignored. So skills ship as a
//! bridge-owned local marketplace (`marketplace.json` + `plugins/<name>/…`)
//! registered in `config.toml`, and Codex installs it into its own
//! `plugins/cache/` on launch — which is why we never write that cache.
//!
//! MCP rides a top-level `[mcp_servers.<slug>]` instead of the plugin bundle so
//! the connector survives even if the plugin/skills path fails. The source tree
//! is content-hashed and left byte-stable when unchanged, so Codex never sees a
//! spurious source change and re-installs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::gateway::manifest::SignedManifest;
use crate::sync::ApplyError;
use crate::sync::host_sync::{HostSync, HostSyncCtx};

use super::config::codex_home;

mod config_toml;
mod marketplace;
mod skills;

use config_toml::write_config_blocks;
use marketplace::{read_existing_version, write_marketplace_json, write_plugin_json};
use skills::{bundle_version, targets_codex, write_skill};

const MARKETPLACE: &str = "systemprompt";
const PLUGIN_NAME: &str = "systemprompt-managed";

#[derive(Clone, Copy, Debug)]
pub struct CodexCliSync;

#[async_trait]
impl HostSync for CodexCliSync {
    fn host_id(&self) -> &'static str {
        "codex-cli"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        let has_content =
            !ctx.manifest.skills.is_empty() || !ctx.manifest.managed_mcp_servers.is_empty();
        if has_content {
            write_marketplace_tree(ctx.manifest)?;
            write_config_blocks(true, &ctx.manifest.managed_mcp_servers)?;
        } else {
            remove_marketplace_tree()?;
            write_config_blocks(false, &[])?;
        }
        Ok(())
    }

    fn clear(&self) -> Result<(), ApplyError> {
        remove_marketplace_tree()?;
        write_config_blocks(false, &[])?;
        Ok(())
    }
}

fn plugin_id() -> String {
    format!("{PLUGIN_NAME}@{MARKETPLACE}")
}

fn marketplace_root() -> PathBuf {
    codex_home().join(".systemprompt").join("marketplace")
}

fn plugin_src_dir() -> PathBuf {
    marketplace_root().join("plugins").join(PLUGIN_NAME)
}

fn cache_plugin_dir() -> PathBuf {
    codex_home()
        .join("plugins")
        .join("cache")
        .join(MARKETPLACE)
        .join(PLUGIN_NAME)
}

fn write_marketplace_tree(manifest: &SignedManifest) -> Result<(), ApplyError> {
    let root = marketplace_root();
    let plugin_dir = plugin_src_dir();
    let version = bundle_version(manifest);

    let source_current = read_existing_version(&plugin_dir).as_deref() == Some(version.as_str())
        && root.join(".agents/plugins/marketplace.json").is_file();
    if !source_current {
        if plugin_dir.exists() {
            fs::remove_dir_all(&plugin_dir)
                .map_err(|e| io_err("clear plugin source", &plugin_dir, e))?;
        }
        fs::create_dir_all(&plugin_dir)
            .map_err(|e| io_err("create plugin source", &plugin_dir, e))?;
        write_marketplace_json(&root)?;
        write_plugin_json(&plugin_dir, &version)?;
        for skill in manifest.skills.iter().filter(|s| targets_codex(s)) {
            write_skill(&plugin_dir, skill)?;
        }
    }

    install_into_cache(&plugin_dir, &version)
}

// Why: Codex marks a plugin installed solely by a version dir under its managed
// cache, so a copy into `cache/<marketplace>/<plugin>/<version>/` suffices.
fn install_into_cache(plugin_dir: &Path, version: &str) -> Result<(), ApplyError> {
    let base = cache_plugin_dir();
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy() == version || !path.is_dir() {
                continue;
            }
            if let Err(e) = fs::remove_dir_all(&path) {
                tracing::debug!(error = %e, path = %path.display(), "leaving stale codex plugin cache dir");
            }
        }
    }

    let dst = base.join(version);
    if read_existing_version(&dst).as_deref() == Some(version) {
        return Ok(());
    }
    if dst.exists() {
        fs::remove_dir_all(&dst).map_err(|e| io_err("clear cache install", &dst, e))?;
    }
    copy_dir_all(plugin_dir, &dst)
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), ApplyError> {
    fs::create_dir_all(dst).map_err(|e| io_err("create", dst, e))?;
    for entry in fs::read_dir(src).map_err(|e| io_err("read dir", src, e))? {
        let entry = entry.map_err(|e| io_err("read entry", src, e))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| io_err("copy", &from, e))?;
        }
    }
    Ok(())
}

fn remove_marketplace_tree() -> Result<(), ApplyError> {
    for dir in [marketplace_root(), cache_plugin_dir()] {
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| io_err("remove", &dir, e))?;
        }
    }
    Ok(())
}

fn io_err(context: &str, path: &Path, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: format!("{context} {}", path.display()),
        source,
    }
}
