//! Verified removal of everything the Claude Code settings module wrote: the
//! key helper, the standalone settings file, and the bridge-owned keys in the
//! user's or machine's settings file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{io_error, render, standalone_settings_path, write_atomic};
use crate::install::mdm::MdmError;

pub(crate) fn remove_managed_settings() -> Result<Vec<String>, MdmError> {
    let standalone = standalone_settings_path().ok_or(MdmError::Resolve("standalone settings"))?;
    let reference = super::read_settings(&standalone)?;
    let fallback = super::key_helper_path().map(|path| super::shell_command_for(&path));
    let helper = reference
        .get("apiKeyHelper")
        .and_then(serde_json::Value::as_str)
        .or(fallback.as_deref())
        .ok_or(MdmError::Resolve("credential helper"))?;
    let paths = [
        Some(crate::config::paths::claude_code_policy_dir().join("managed-settings.json")),
        crate::config::paths::claude_cli_settings_path(),
    ];
    let mut lines = Vec::new();
    for path in paths.into_iter().flatten() {
        let Some(existing) =
            crate::fsutil::read_optional(&path).map_err(io_error("read", &path))?
        else {
            continue;
        };
        if existing.trim().is_empty() {
            continue;
        }
        let mut root: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&existing)
            .map_err(|source| MdmError::Json {
                path: path.clone(),
                source,
            })?;
        if root.get("apiKeyHelper").and_then(serde_json::Value::as_str) != Some(helper) {
            continue;
        }
        root.remove("apiKeyHelper");
        super::model_picker::strip_owned_rows(&mut root)?;
        if let Some(serde_json::Value::Object(env)) = root.get_mut("env") {
            for key in [
                "ANTHROPIC_BASE_URL",
                "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY",
                "CLAUDE_CODE_ATTRIBUTION_HEADER",
            ] {
                if reference.get("env").and_then(|value| value.get(key)) == env.get(key) {
                    env.remove(key);
                }
            }
            if env.is_empty() {
                root.remove("env");
            }
        }
        if root.is_empty() {
            crate::fsutil::remove_verified(&path).map_err(io_error("remove", &path))?;
            lines.push(format!("removed: {}", path.display()));
        } else {
            write_atomic(&path, &render(root, &path)?)?;
            lines.push(format!("cleaned: {} (bridge keys)", path.display()));
        }
    }
    Ok(lines)
}

pub(crate) fn remove_all() -> Result<Vec<String>, MdmError> {
    let mut lines = remove_managed_settings()?;
    let paths = [
        #[cfg(unix)]
        super::key_helper_path().ok_or(MdmError::Resolve("helper path"))?,
        standalone_settings_path().ok_or(MdmError::Resolve("the user's config directory"))?,
    ];
    for path in paths {
        if path.try_exists().map_err(io_error("read", &path))? {
            crate::fsutil::remove_verified(&path).map_err(io_error("remove", &path))?;
            lines.push(format!("removed: {}", path.display()));
        }
    }
    super::model_picker::remove_sidecar()?;
    Ok(lines)
}
