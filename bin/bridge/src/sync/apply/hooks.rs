//! Hook materialisation from manifest entries, gated on recorded consent.
//!
//! Claude Code runs plugin hooks session-globally: a `PreToolUse` hook with a
//! `*` matcher fires on every tool call regardless of which plugin contributed
//! the tool. Materialising the governance hooks into every plugin would
//! therefore fire N identical-but-not-deduplicated calls per tool call (the
//! `?plugin_id=` query differs, so Claude Code's identical-command dedup does
//! not apply). Exactly one plugin carries them — the one whose config sets
//! `hooks.governance` — and every other plugin gets an empty hooks file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ApplyError;
use super::hooks_schema::{HookEntry as WireHookEntry, HooksFile};
use crate::gateway::manifest::{HookEntry as ManifestHookEntry, PluginEntry};
use std::fs;
use std::path::Path;

pub(super) fn write_hooks_json(
    plugin: &PluginEntry,
    plugin_dir: &Path,
    hook_pool: &[ManifestHookEntry],
) -> Result<(), ApplyError> {
    let plugin_id = &plugin.id;
    let hooks_dir = plugin_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).map_err(|e| ApplyError::Io {
        context: format!("create {}", hooks_dir.display()),
        source: e,
    })?;

    let body = if plugin.hooks.is_empty() {
        HooksFile::empty()
    } else {
        build_hooks_file(plugin, hook_pool)?
    };

    let bytes = serde_json::to_vec_pretty(&body).map_err(|e| ApplyError::Serialize {
        what: format!("hooks.json for {plugin_id}"),
        source: e,
    })?;
    let path = hooks_dir.join("hooks.json");
    fs::write(&path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })?;
    Ok(())
}

fn build_hooks_file(
    plugin: &PluginEntry,
    hook_pool: &[ManifestHookEntry],
) -> Result<HooksFile, ApplyError> {
    let plugin_id = &plugin.id;
    let authorization = crate::proxy::loopback_bearer().map_err(|e| ApplyError::Io {
        context: format!("loopback secret for hooks.json ({plugin_id})"),
        source: e,
    })?;
    let origin = crate::proxy::loopback_origin();

    let mut body = if plugin.hooks.governance {
        let govern_url = format!("{origin}/api/public/hooks/govern?plugin_id={plugin_id}");
        let track_url = format!("{origin}/api/public/hooks/track?plugin_id={plugin_id}");
        HooksFile::new(govern_url, &track_url, &authorization)
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

/// Normalises a synced plugin.json for the managed contract: the hooks
/// pointer, and `installationPreference` without which Cowork's MDM path shows
/// "Contact an organization owner" instead of auto-installing.
pub(super) fn ensure_plugin_json_managed_fields(plugin_dir: &Path) -> Result<(), ApplyError> {
    let Some(path) = super::plugin_manifest_path(plugin_dir) else {
        return Ok(());
    };
    let bytes = fs::read(&path).map_err(|e| ApplyError::Io {
        context: format!("read {}", path.display()),
        source: e,
    })?;
    let mut value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let Some(obj) = value.as_object_mut() else {
        return Ok(());
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
        return Ok(());
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
    fs::write(&path, next).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}
