//! Verified removal of everything the Claude Code settings module wrote: the
//! key helper, the standalone settings file, and the bridge-owned keys in the
//! user's or machine's settings file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    io_error, key_helper_path, managed_settings_path, render, standalone_settings_path,
    write_atomic,
};
use crate::install::mdm::MdmError;

pub(crate) fn remove_managed_settings() -> Result<Vec<String>, MdmError> {
    let path = managed_settings_path().ok_or(MdmError::Resolve("managed settings path"))?;
    let Some(existing) = crate::fsutil::read_optional(&path).map_err(io_error("read", &path))?
    else {
        return Ok(Vec::new());
    };
    let mut root: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&existing)
        .map_err(|source| MdmError::Json {
            path: path.clone(),
            source,
        })?;
    root.remove("apiKeyHelper");
    super::model_picker::strip_owned_rows(&mut root);
    if let Some(serde_json::Value::Object(env)) = root.get_mut("env") {
        for key in [
            "ANTHROPIC_BASE_URL",
            "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY",
            "CLAUDE_CODE_ATTRIBUTION_HEADER",
        ] {
            env.remove(key);
        }
        if env.is_empty() {
            root.remove("env");
        }
    }
    if root.is_empty() {
        crate::fsutil::remove_verified(&path).map_err(io_error("remove", &path))?;
        return Ok(vec![format!("removed: {}", path.display())]);
    }
    write_atomic(&path, &render(root, &path)?)?;
    Ok(vec![format!("cleaned: {} (bridge keys)", path.display())])
}

pub(crate) fn remove_all() -> Result<Vec<String>, MdmError> {
    let mut lines = Vec::new();
    for path in [
        key_helper_path().ok_or(MdmError::Resolve("helper path"))?,
        standalone_settings_path().ok_or(MdmError::Resolve("the user's config directory"))?,
    ] {
        if path.try_exists().map_err(io_error("read", &path))? {
            crate::fsutil::remove_verified(&path).map_err(io_error("remove", &path))?;
            lines.push(format!("removed: {}", path.display()));
        }
    }
    lines.extend(remove_managed_settings()?);
    super::model_picker::remove_sidecar()?;
    Ok(lines)
}
