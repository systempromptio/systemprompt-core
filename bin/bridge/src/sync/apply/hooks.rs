use super::ApplyError;
use super::hooks_schema::{HookEntry as WireHookEntry, HooksFile};
use crate::gateway::manifest::HookEntry as ManifestHookEntry;
use std::fs;
use std::path::Path;
use systemprompt_identifiers::PluginId;

// Hooks route through the bridge loopback proxy (not the gateway directly).
// Cowork presents the static loopback secret; the proxy strips it and injects
// the plugin's `aud:hook` gateway token (minted on demand from `plugin_id` in
// the query). This replaces the old `.env.plugin` +
// `$SYSTEMPROMPT_PLUGIN_TOKEN` env-var delivery, which Cowork's agent VM did
// not reliably propagate.
pub(super) fn write_hooks_json(
    plugin_id: &PluginId,
    plugin_dir: &Path,
    user_hooks: &[ManifestHookEntry],
) -> Result<(), ApplyError> {
    let hooks_dir = plugin_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).map_err(|e| ApplyError::Io {
        context: format!("create {}", hooks_dir.display()),
        source: e,
    })?;
    let authorization = crate::proxy::loopback_bearer().map_err(|e| ApplyError::Io {
        context: format!("loopback secret for hooks.json ({plugin_id})"),
        source: e,
    })?;
    let origin = crate::proxy::loopback_origin();
    let govern_url = format!("{origin}/api/public/hooks/govern?plugin_id={plugin_id}");
    let track_url = format!("{origin}/api/public/hooks/track?plugin_id={plugin_id}");
    let mut body = HooksFile::new(govern_url, &track_url, &authorization);
    for hook in user_hooks {
        let entry =
            WireHookEntry::user_command(hook.command.clone(), hook.event.as_str(), hook.is_async);
        body.append_user_hook(hook.event.as_str().to_string(), hook.matcher.clone(), entry);
    }
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

pub(super) fn ensure_plugin_json_hooks_field(plugin_dir: &Path) -> Result<(), ApplyError> {
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
    let already = obj
        .get("hooks")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|s| s == "./hooks/hooks.json");
    if already {
        return Ok(());
    }
    obj.insert(
        "hooks".to_string(),
        serde_json::Value::String("./hooks/hooks.json".to_string()),
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
