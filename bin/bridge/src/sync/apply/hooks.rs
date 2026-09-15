//! Hook materialisation from manifest entries, gated on recorded consent.
//!
//! Claude Code runs plugin hooks session-globally: a `PreToolUse` hook with a
//! `*` matcher fires on every tool call regardless of which plugin contributed
//! the tool. Materialising the governance hooks into every plugin would
//! therefore fire N identical-but-not-deduplicated calls per tool call (the
//! `?plugin_id=` query differs, so Claude Code's identical-command dedup does
//! not apply). Exactly one plugin carries them — the one whose config sets
//! `hooks.governance` — and every other plugin gets an empty hooks file. The
//! comms drain hooks ride on the same owner, and only when it also sets
//! `hooks.comms`.
//!
//! `hooks.json` lives in the org-plugins tree, which other local accounts can
//! read, so it carries the per-plugin hook token derived from the loopback
//! secret — never the secret itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ApplyError;
use crate::fsutil::{FileReceipt, atomic_write_0644};
use crate::gateway::manifest::{HookEntry as ManifestHookEntry, PluginEntry};
use crate::host_sync::hooks_schema::{HookEntry as WireHookEntry, HooksFile};
use crate::proxy::LoopbackEndpoint;
use crate::proxy::scoped_token::hook_token;
use std::fs;
use std::path::Path;

pub(super) fn write_hooks_json(
    loopback: &LoopbackEndpoint,
    plugin: &PluginEntry,
    plugin_dir: &Path,
    hook_pool: &[ManifestHookEntry],
) -> Result<FileReceipt, ApplyError> {
    let plugin_id = &plugin.id;
    let hooks_dir = plugin_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).map_err(|e| ApplyError::Io {
        context: format!("create {}", hooks_dir.display()),
        source: e,
    })?;

    let body = if plugin.hooks.is_empty() {
        HooksFile::empty()
    } else {
        build_hooks_file(loopback, plugin, hook_pool)?
    };

    let bytes = serde_json::to_vec_pretty(&body).map_err(|e| ApplyError::Serialize {
        what: format!("hooks.json for {plugin_id}"),
        source: e,
    })?;
    let path = hooks_dir.join("hooks.json");
    atomic_write_0644(&path, &bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })?;
    FileReceipt::verify(&path, &bytes).map_err(|e| ApplyError::Io {
        context: format!("verify {}", path.display()),
        source: e,
    })
}

fn comms_drain_command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!("\"{}\" comms-drain", exe.display()))
}

fn build_hooks_file(
    loopback: &LoopbackEndpoint,
    plugin: &PluginEntry,
    hook_pool: &[ManifestHookEntry],
) -> Result<HooksFile, ApplyError> {
    let plugin_id = &plugin.id;
    let secret = loopback.secret_or_mint().map_err(|e| ApplyError::Io {
        context: format!("loopback secret for hooks.json ({plugin_id})"),
        source: e,
    })?;
    let token = hook_token(&secret, plugin_id);
    let authorization = format!("Bearer {}", token.as_str());
    let origin = loopback.origin();

    let mut body = if plugin.hooks.governance {
        let govern_url = format!("{origin}/api/public/hooks/govern?plugin_id={plugin_id}");
        let track_url = format!("{origin}/api/public/hooks/track?plugin_id={plugin_id}");
        let mut file = HooksFile::new(govern_url, &track_url, &authorization);
        if plugin.hooks.comms
            && let Some(command) = comms_drain_command()
        {
            file.append_comms_hooks(&command);
        }
        file
    } else {
        HooksFile::empty()
    };

    for id in &plugin.hooks.include {
        let Some(hook) = hook_pool.iter().find(|h| h.id.as_str() == id) else {
            tracing::warn!(
                plugin_id = %plugin_id,
                hook_id = %id,
                "plugin references a hook that is not in the manifest; skipping"
            );
            continue;
        };
        let entry =
            WireHookEntry::user_command(hook.command.clone(), hook.event.as_str(), hook.is_async);
        body.append_user_hook(hook.event.as_str().to_owned(), hook.matcher.clone(), entry);
    }
    Ok(body)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PluginJsonShape {
    Stamped,
    Absent,
    Malformed(String),
}

// Why: Cowork auto-installs a managed plugin only when its manifest sets
// `installationPreference`.
pub(super) fn ensure_plugin_json_managed_fields(
    plugin_dir: &Path,
) -> Result<PluginJsonShape, ApplyError> {
    let Some(path) = super::plugin_manifest_path(plugin_dir) else {
        return Ok(PluginJsonShape::Absent);
    };
    let bytes = fs::read(&path).map_err(|e| ApplyError::Io {
        context: format!("read {}", path.display()),
        source: e,
    })?;
    // JSON: plugin.json is the host's own manifest; only the two managed
    // fields are stamped and everything else passes through untouched.
    let mut value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(e) => {
            return Ok(PluginJsonShape::Malformed(format!(
                "{} is not valid JSON: {e}",
                path.display()
            )));
        },
    };
    let Some(obj) = value.as_object_mut() else {
        return Ok(PluginJsonShape::Malformed(format!(
            "{} is not a JSON object",
            path.display()
        )));
    };
    let hooks_ok = obj
        .get("hooks")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|s| s == "./hooks/hooks.json");
    let pref_ok = obj
        .get("installationPreference")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|s| s == super::PLUGIN_INSTALLATION_PREFERENCE);
    if hooks_ok && pref_ok {
        return Ok(PluginJsonShape::Stamped);
    }
    obj.insert(
        "hooks".to_owned(),
        serde_json::Value::String("./hooks/hooks.json".to_owned()),
    );
    obj.insert(
        "installationPreference".to_owned(),
        serde_json::Value::String(super::PLUGIN_INSTALLATION_PREFERENCE.to_owned()),
    );
    let next = serde_json::to_vec_pretty(&value).map_err(|e| ApplyError::Serialize {
        what: "plugin.json".into(),
        source: e,
    })?;
    atomic_write_0644(&path, &next).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })?;
    FileReceipt::verify(&path, &next)
        .map(|_| PluginJsonShape::Stamped)
        .map_err(|e| ApplyError::Io {
            context: format!("verify {}", path.display()),
            source: e,
        })
}
