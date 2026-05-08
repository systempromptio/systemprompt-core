use super::ApplyError;
use super::hooks_schema::{HookEntry as WireHookEntry, HooksFile};
use crate::auth::plugin_oauth::mint_or_refresh_plugin_token;
use crate::fsutil;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::HookEntry as ManifestHookEntry;
use std::fs;
use std::path::Path;

const PLUGIN_TOKEN_ENV_VAR: &str = "SYSTEMPROMPT_PLUGIN_TOKEN";

// Atomic 0600 write — the file holds a bearer token, must not leak between
// users on multi-tenant hosts.
pub(crate) async fn materialize_hook_token(
    client: &GatewayClient,
    bearer: &str,
    plugin_id: &str,
    plugin_dir: &Path,
) -> Result<(), ApplyError> {
    let token = mint_or_refresh_plugin_token(client, bearer, plugin_id).await?;
    let env_path = plugin_dir.join(".env.plugin");
    let body = format!("{PLUGIN_TOKEN_ENV_VAR}={}\n", token.access_token);
    fsutil::atomic_write_0600(&env_path, body.as_bytes()).map_err(|e| ApplyError::Io {
        context: format!(".env.plugin for {plugin_id}"),
        source: e,
    })
}

pub(crate) fn write_hooks_json(
    gateway_base: &str,
    plugin_id: &str,
    plugin_dir: &Path,
    user_hooks: &[ManifestHookEntry],
) -> Result<(), ApplyError> {
    let hooks_dir = plugin_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).map_err(|e| ApplyError::Io {
        context: format!("create {}", hooks_dir.display()),
        source: e,
    })?;
    let base = gateway_base.trim_end_matches('/');
    let govern_url = format!("{base}/api/public/hooks/govern?plugin_id={plugin_id}");
    let track_url = format!("{base}/api/public/hooks/track?plugin_id={plugin_id}");
    let mut body = HooksFile::new(govern_url, &track_url, PLUGIN_TOKEN_ENV_VAR);
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

pub(crate) fn ensure_plugin_json_hooks_field(plugin_dir: &Path) -> Result<(), ApplyError> {
    let path = plugin_dir.join("claude-plugin").join("plugin.json");
    if !path.is_file() {
        return Ok(());
    }
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
        .map(|s| s == "./hooks/hooks.json")
        .unwrap_or(false);
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
