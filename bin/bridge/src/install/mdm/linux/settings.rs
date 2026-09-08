//! Claude Code settings on Linux.
//!
//! The `apiKeyHelper` script plus the `env` keys the bridge owns inside the
//! settings file: `/etc/claude-code/managed-settings.json` when running as
//! root, otherwise the per-user `~/.claude/settings.json`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use super::{io_error, read_or_empty, write_atomic};
use crate::install::mdm::MdmError;

// Why: Claude Code reads ~/.claude/settings.json, not
// ~/.claude/managed-settings.json.
fn managed_settings_path() -> Option<PathBuf> {
    let system = PathBuf::from("/etc/claude-code/managed-settings.json");
    if can_write(&system) {
        return Some(system);
    }
    Some(
        crate::basedirs::home_dir()?
            .join(".claude")
            .join("settings.json"),
    )
}

fn can_write(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    fs::create_dir_all(parent).is_ok()
        && fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .is_ok()
}

pub(super) fn key_helper_path() -> Option<PathBuf> {
    Some(
        crate::basedirs::config_dir()?
            .join(crate::brand::brand().config_dir)
            .join("claude-key-helper.sh"),
    )
}

fn key_helper_body(key_path: &Path) -> String {
    let bin = crate::brand::brand().binary_name;
    format!(
        "#!/bin/sh\n\
         # Written by `{bin} install --apply`. Rewritten on every apply — do not edit.\n\
         exec cat \"{key}\"\n",
        key = key_path.display(),
    )
}

pub(super) fn apply_managed_settings(
    gateway: &str,
    key_path: &Path,
) -> Result<super::super::MdmApplication, MdmError> {
    let helper = key_helper_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let helper_body = key_helper_body(key_path);
    write_atomic(&helper, &helper_body)?;
    set_executable(&helper)?;

    let mut files = vec![
        crate::fsutil::FileReceipt::verify(&helper, helper_body.as_bytes())
            .map_err(io_error("verify helper", &helper))?,
    ];
    let outcome = (|| {
        let settings_path =
            managed_settings_path().ok_or(MdmError::Resolve("the managed settings path"))?;
        let existing = read_or_empty(&settings_path)?;
        let mut root: serde_json::Map<String, serde_json::Value> = if existing.trim().is_empty() {
            serde_json::Map::new()
        } else {
            serde_json::from_str(&existing).map_err(|e| MdmError::Json {
                path: settings_path.clone(),
                source: e,
            })?
        };

        let mut lines = vec![format!("wrote: {} (apiKeyHelper)", helper.display())];
        let conflicts = warn_on_forced_login(&root);
        if !conflicts.is_empty() {
            return Err(MdmError::InvalidConfig(conflicts.join("; ")));
        }

        let env = root
            .entry("env".to_owned())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        let Some(env) = env.as_object_mut() else {
            return Err(MdmError::EnvNotObject {
                path: settings_path,
            });
        };
        env.insert(
            "ANTHROPIC_BASE_URL".to_owned(),
            serde_json::Value::String(gateway.to_owned()),
        );
        env.insert(
            "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY".to_owned(),
            serde_json::Value::String("1".to_owned()),
        );
        // Why: Claude Code's attribution header reaches non-Anthropic providers as
        // system-prompt content.
        env.insert(
            "CLAUDE_CODE_ATTRIBUTION_HEADER".to_owned(),
            serde_json::Value::String("0".to_owned()),
        );

        root.insert(
            "apiKeyHelper".to_owned(),
            serde_json::Value::String(helper.display().to_string()),
        );

        let rendered =
            serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|e| {
                MdmError::Json {
                    path: settings_path.clone(),
                    source: e,
                }
            })?;
        let body = format!("{rendered}\n");
        write_atomic(&settings_path, &body)?;
        files.push(
            crate::fsutil::FileReceipt::verify(&settings_path, body.as_bytes())
                .map_err(io_error("verify settings", &settings_path))?,
        );
        lines.push(format!(
            "wrote: {} (ANTHROPIC_BASE_URL, apiKeyHelper, model discovery)",
            settings_path.display()
        ));
        Ok::<_, MdmError>(lines)
    })();
    let lines = outcome.map_err(|source| MdmError::Partial {
        completed: super::super::MdmApplication {
            files: files.clone(),
            ..Default::default()
        },
        source: Box::new(source),
    })?;
    Ok(super::super::MdmApplication {
        lines,
        files,
        policies: Vec::new(),
    })
}

// Why: Claude Code v2.1.146+ forceLoginMethod/forceLoginOrgUUID block API keys
// and apiKeyHelper.
fn warn_on_forced_login(root: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    ["forceLoginMethod", "forceLoginOrgUUID"]
        .into_iter()
        .filter(|key| root.contains_key(*key))
        .map(|key| {
            format!(
                "WARNING: managed settings already set \"{key}\", which blocks the gateway \
                 credential at startup — remove it or Claude Code will refuse to run"
            )
        })
        .collect()
}

pub(super) fn remove_managed_settings() -> Result<Vec<String>, MdmError> {
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
    let rendered =
        serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|source| {
            MdmError::Json {
                path: path.clone(),
                source,
            }
        })?;
    write_atomic(&path, &format!("{rendered}\n"))?;
    Ok(vec![format!("cleaned: {} (bridge keys)", path.display())])
}

fn set_executable(path: &Path) -> Result<(), MdmError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(io_error("chmod", path))?;
    if fs::metadata(path)
        .map_err(io_error("verify chmod", path))?
        .permissions()
        .mode()
        & 0o777
        != 0o700
    {
        return Err(MdmError::InvalidConfig(format!(
            "{}: expected mode 0700",
            path.display()
        )));
    }
    Ok(())
}

// Why: Claude Code stores the user's /model selection in this same settings
// file.
pub(crate) fn seed_default_model(model: &str) -> Result<bool, MdmError> {
    let settings_path =
        managed_settings_path().ok_or(MdmError::Resolve("the managed settings path"))?;
    let existing = read_or_empty(&settings_path)?;
    let mut root: serde_json::Map<String, serde_json::Value> = if existing.trim().is_empty() {
        serde_json::Map::new()
    } else {
        serde_json::from_str(&existing).map_err(|e| MdmError::Json {
            path: settings_path.clone(),
            source: e,
        })?
    };
    if root.contains_key("model") {
        return Ok(false);
    }
    root.insert(
        "model".to_owned(),
        serde_json::Value::String(model.to_owned()),
    );
    let rendered = serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|e| {
        MdmError::Json {
            path: settings_path.clone(),
            source: e,
        }
    })?;
    write_atomic(&settings_path, &format!("{rendered}\n"))?;
    Ok(true)
}

#[path = "settings_test_api.rs"]
pub mod test_api;
