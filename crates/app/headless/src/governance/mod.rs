//! Governance wiring handed to a host: the hooks file that routes each
//! tool call through the governance pipeline.
//!
//! For Claude Code, [`hooks_file`] materialises a run-local plugin under
//! `RunLayout::host` whose `PreToolUse` hook asks `govern` for a verdict and
//! whose lifecycle hooks report to `track`, all authorised with the run's
//! hook token. Other hosts have no hooks surface; they get `None` and the
//! caller records that the run is ungoverned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod hooks_schema;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use systemprompt_identifiers::PluginId;

use self::hooks_schema::{HookMatcher, HooksFile, HttpHook, PluginManifest, TRACK_EVENTS};
use crate::error::{HeadlessError, HeadlessResult};
use crate::layout::RunLayout;
use crate::request::HostKind;

pub const GOVERNANCE_PLUGIN_NAME: &str = "headless-governance";
const GOVERNANCE_PLUGIN_DIR: &str = "governance-plugin";

/// The governance endpoints the hooks call, e.g.
/// `https://host/api/public/hooks/govern`; `?plugin_id=` is appended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookBases {
    pub govern_url: String,
    pub track_url: String,
}

#[must_use]
pub fn hooks_document(plugin_id: &PluginId, token: &str, bases: &HookBases) -> HooksFile {
    let govern = with_plugin(&bases.govern_url, plugin_id);
    let track = with_plugin(&bases.track_url, plugin_id);
    let mut hooks = BTreeMap::new();
    hooks.insert(
        "PreToolUse".to_owned(),
        vec![HookMatcher::any(HttpHook::govern(govern, token))],
    );
    for event in TRACK_EVENTS {
        hooks.insert(
            (*event).to_owned(),
            vec![HookMatcher::any(HttpHook::track(
                track.clone(),
                token,
                event,
            ))],
        );
    }
    HooksFile { hooks }
}

pub async fn hooks_file(
    host: HostKind,
    plugin_id: &PluginId,
    token: &str,
    bases: &HookBases,
    layout: &RunLayout,
) -> HeadlessResult<Option<PathBuf>> {
    if host != HostKind::ClaudeCode {
        return Ok(None);
    }
    let dir = layout.host.join(GOVERNANCE_PLUGIN_DIR);
    let manifest = PluginManifest {
        name: GOVERNANCE_PLUGIN_NAME.to_owned(),
        description: "Reports this headless run to the governance spine.".to_owned(),
        version: "1.0.0".to_owned(),
    };
    write_json(&dir.join(".claude-plugin/plugin.json"), &manifest).await?;
    write_json(
        &dir.join("hooks/hooks.json"),
        &hooks_document(plugin_id, token, bases),
    )
    .await?;
    Ok(Some(dir))
}

fn with_plugin(base: &str, plugin_id: &PluginId) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}plugin_id={}", plugin_id.as_str())
}

async fn write_json<T: Serialize + Sync>(path: &Path, value: &T) -> HeadlessResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let hooks_error = |source| HeadlessError::HooksWrite {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(hooks_error)?;
    }
    tokio::fs::write(path, bytes).await.map_err(hooks_error)
}
