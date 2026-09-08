//! Claude Code (terminal CLI) settings on macOS and Linux.
//!
//! The `apiKeyHelper` script plus the `env` keys the bridge owns inside the
//! settings file: the machine policy file under
//! [`crate::config::paths::claude_code_policy_dir`] when it is writable
//! (root), otherwise the per-user `~/.claude/settings.json`.
//!
//! Why this is not Linux-only: Claude Desktop is configured through managed
//! preferences, but the Claude Code CLI reads none of them. Before this module
//! was shared, `install --apply` on macOS wrote the Desktop plist and reported
//! Claude Code as governed, while every `claude` model call still went straight
//! to Anthropic and never reached the audit trail.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use crate::install::mdm::MdmError;

// Why: Claude Code reads ~/.claude/settings.json, not
// ~/.claude/managed-settings.json.
fn managed_settings_path() -> Option<PathBuf> {
    let system = crate::config::paths::claude_code_policy_dir().join("managed-settings.json");
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

/// The bridge-owned Claude Code keys as a settings file of their own, next to
/// the key helper. `claude --settings <this file>` routes one session through
/// the gateway without touching `~/.claude/settings.json`, so a developer who
/// keeps their own Anthropic login can switch per terminal.
pub(super) fn standalone_settings_path() -> Option<PathBuf> {
    Some(
        crate::basedirs::config_dir()?
            .join(crate::brand::brand().config_dir)
            .join("claude-code-settings.json"),
    )
}

fn bridge_env(gateway: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut env = serde_json::Map::new();
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
    env
}

/// Writes the key helper and the standalone settings fragment. Every apply
/// path calls this, whether or not the user also asked for the merge into
/// their own settings file.
pub(super) fn write_standalone_settings(
    gateway: &str,
    key_path: &Path,
) -> Result<Vec<String>, MdmError> {
    let helper = key_helper_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    write_atomic(&helper, &key_helper_body(key_path))?;
    set_executable(&helper)?;
    let standalone =
        standalone_settings_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let mut root = serde_json::Map::new();
    root.insert(
        "env".to_owned(),
        serde_json::Value::Object(bridge_env(gateway)),
    );
    root.insert(
        "apiKeyHelper".to_owned(),
        serde_json::Value::String(shell_command_for(&helper)),
    );
    let rendered = serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|e| {
        MdmError::Json {
            path: standalone.clone(),
            source: e,
        }
    })?;
    write_atomic(&standalone, &format!("{rendered}\n"))?;
    Ok(vec![
        format!("wrote: {} (apiKeyHelper)", helper.display()),
        format!(
            "wrote: {} (pass to `claude --settings` to route one session without editing \
             ~/.claude/settings.json)",
            standalone.display()
        ),
    ])
}

pub(crate) fn apply_managed_settings(
    gateway: &str,
    key_path: &Path,
) -> Result<Vec<String>, MdmError> {
    let helper = key_helper_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let mut lines = write_standalone_settings(gateway, key_path)?;

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

    lines.extend(warn_on_forced_login(&root));

    let env = root
        .entry("env".to_owned())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(env) = env.as_object_mut() else {
        return Err(MdmError::EnvNotObject {
            path: settings_path,
        });
    };
    env.extend(bridge_env(gateway));

    root.insert(
        "apiKeyHelper".to_owned(),
        serde_json::Value::String(shell_command_for(&helper)),
    );

    let rendered = serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|e| {
        MdmError::Json {
            path: settings_path.clone(),
            source: e,
        }
    })?;
    write_atomic(&settings_path, &format!("{rendered}\n"))?;
    lines.push(format!(
        "wrote: {} (ANTHROPIC_BASE_URL, apiKeyHelper, model discovery)",
        settings_path.display()
    ));
    Ok(lines)
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

pub(crate) fn remove_managed_settings() -> Vec<String> {
    let Some(path) = managed_settings_path() else {
        return Vec::new();
    };
    let Ok(existing) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Object(mut root)) = serde_json::from_str(&existing) else {
        return vec![format!("left {} in place: not valid JSON", path.display())];
    };
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
        return match fs::remove_file(&path) {
            Ok(()) => vec![format!("removed: {}", path.display())],
            Err(e) => vec![format!("could not remove {}: {e}", path.display())],
        };
    }
    let Ok(rendered) = serde_json::to_string_pretty(&serde_json::Value::Object(root)) else {
        return Vec::new();
    };
    match write_atomic(&path, &format!("{rendered}\n")) {
        Ok(()) => vec![format!("cleaned: {} (bridge keys)", path.display())],
        Err(e) => vec![format!("could not clean {}: {e}", path.display())],
    }
}

/// Removes everything this module wrote: the key helper and the bridge-owned
/// keys in the settings file. One line per action, empty when nothing was ours.
pub(super) fn remove_all() -> Vec<String> {
    let mut lines = Vec::new();
    for path in [key_helper_path(), standalone_settings_path()]
        .into_iter()
        .flatten()
    {
        match fs::remove_file(&path) {
            Ok(()) => lines.push(format!("removed: {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => lines.push(format!("could not remove {}: {e}", path.display())),
        }
    }
    lines.extend(remove_managed_settings());
    lines
}

pub(super) fn io_error(
    action: &'static str,
    path: &Path,
) -> impl FnOnce(std::io::Error) -> MdmError {
    let path = path.to_path_buf();
    move |source| MdmError::Io {
        action,
        path,
        source,
    }
}

pub(super) fn write_atomic(path: &Path, contents: &str) -> Result<(), MdmError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error("create", parent))?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, contents).map_err(io_error("write", &tmp))?;
    fs::rename(&tmp, path).map_err(|e| {
        _ = fs::remove_file(&tmp);
        io_error("rename onto", path)(e)
    })
}

pub(super) fn read_or_empty(path: &Path) -> Result<String, MdmError> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(io_error("read", path)(e)),
    }
}

// Why: Claude Code hands `apiKeyHelper` to `/bin/sh` verbatim, so a path with
// whitespace — every macOS `~/Library/Application Support/…` path — is split
// into words and fails with "No such file or directory". Quote only then, so
// the Linux value stays the bare path existing installs and tests expect.
fn shell_command_for(helper: &Path) -> String {
    let raw = helper.display().to_string();
    if raw.chars().any(char::is_whitespace) {
        format!("'{}'", raw.replace('\'', "'\\''"))
    } else {
        raw
    }
}

fn set_executable(path: &Path) -> Result<(), MdmError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(io_error("chmod", path))
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

#[path = "claude_code_settings_test_api.rs"]
pub mod test_api;
