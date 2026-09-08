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

mod removal;

use std::fs;
use std::path::{Path, PathBuf};

use crate::fsutil::FileReceipt;
use crate::install::mdm::{MdmApplication, MdmError};

pub(crate) use self::removal::{remove_all, remove_managed_settings};

// Why: Claude Code reads ~/.claude/settings.json, not
// ~/.claude/managed-settings.json.
pub fn managed_settings_path() -> Option<PathBuf> {
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

pub(super) fn standalone_settings_path() -> Option<PathBuf> {
    Some(
        crate::basedirs::config_dir()?
            .join(crate::brand::brand().config_dir)
            .join("claude-code-settings.json"),
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

fn render(
    root: serde_json::Map<String, serde_json::Value>,
    path: &Path,
) -> Result<String, MdmError> {
    let rendered = serde_json::to_string_pretty(&serde_json::Value::Object(root)).map_err(|e| {
        MdmError::Json {
            path: path.to_path_buf(),
            source: e,
        }
    })?;
    Ok(format!("{rendered}\n"))
}

fn write_verified(path: &Path, body: &str) -> Result<FileReceipt, MdmError> {
    write_atomic(path, body)?;
    FileReceipt::verify(path, body.as_bytes()).map_err(io_error("verify", path))
}

// Why: the standalone file lets `claude --settings <file>` route one session
// through the gateway without touching `~/.claude/settings.json`, so a
// developer who keeps their own Anthropic login can switch per terminal.
pub(crate) fn write_standalone_settings(
    gateway: &str,
    key_path: &Path,
) -> Result<MdmApplication, MdmError> {
    let helper = key_helper_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let helper_receipt = write_verified(&helper, &key_helper_body(key_path))?;
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
    let standalone_receipt = write_verified(&standalone, &render(root, &standalone)?)?;
    Ok(MdmApplication {
        lines: vec![
            format!("wrote: {} (apiKeyHelper)", helper.display()),
            format!(
                "wrote: {} (pass to `claude --settings` to route one session without editing \
                 ~/.claude/settings.json)",
                standalone.display()
            ),
        ],
        files: vec![helper_receipt, standalone_receipt],
        policies: Vec::new(),
    })
}

pub(crate) fn apply_managed_settings(
    gateway: &str,
    key_path: &Path,
) -> Result<MdmApplication, MdmError> {
    let MdmApplication {
        mut lines,
        mut files,
        ..
    } = write_standalone_settings(gateway, key_path)?;
    let helper = key_helper_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let outcome = (|| {
        let settings_path =
            managed_settings_path().ok_or(MdmError::Resolve("the managed settings path"))?;
        let mut root = read_settings(&settings_path)?;
        merge_bridge_keys(&mut root, &settings_path, gateway, &helper)?;
        files.push(write_verified(
            &settings_path,
            &render(root, &settings_path)?,
        )?);
        lines.push(format!(
            "wrote: {} (ANTHROPIC_BASE_URL, apiKeyHelper, model discovery)",
            settings_path.display()
        ));
        Ok::<_, MdmError>(())
    })();
    outcome.map_err(|source| MdmError::Partial {
        completed: MdmApplication {
            lines: lines.clone(),
            files: files.clone(),
            policies: Vec::new(),
        },
        source: Box::new(source),
    })?;
    Ok(MdmApplication {
        lines,
        files,
        policies: Vec::new(),
    })
}

fn merge_bridge_keys(
    root: &mut serde_json::Map<String, serde_json::Value>,
    settings_path: &Path,
    gateway: &str,
    helper: &Path,
) -> Result<(), MdmError> {
    let conflicts = forced_login_conflicts(root);
    if !conflicts.is_empty() {
        return Err(MdmError::InvalidConfig(conflicts.join("; ")));
    }
    let env = root
        .entry("env".to_owned())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(env) = env.as_object_mut() else {
        return Err(MdmError::EnvNotObject {
            path: settings_path.to_path_buf(),
        });
    };
    env.extend(bridge_env(gateway));
    root.insert(
        "apiKeyHelper".to_owned(),
        serde_json::Value::String(shell_command_for(helper)),
    );
    Ok(())
}

fn read_settings(path: &Path) -> Result<serde_json::Map<String, serde_json::Value>, MdmError> {
    let existing = read_or_empty(path)?;
    if existing.trim().is_empty() {
        return Ok(serde_json::Map::new());
    }
    serde_json::from_str(&existing).map_err(|e| MdmError::Json {
        path: path.to_path_buf(),
        source: e,
    })
}

// Why: Claude Code v2.1.146+ forceLoginMethod/forceLoginOrgUUID block API keys
// and apiKeyHelper.
fn forced_login_conflicts(root: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
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
    crate::fsutil::atomic_write_0644(path, contents.as_bytes())
        .map_err(io_error("write and verify", path))
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
// into words. Quote only then, so the Linux value stays the bare path.
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
pub fn seed_default_model(model: &str) -> Result<bool, MdmError> {
    let settings_path =
        managed_settings_path().ok_or(MdmError::Resolve("the managed settings path"))?;
    let mut root = read_settings(&settings_path)?;
    if root.contains_key("model") {
        return Ok(false);
    }
    root.insert(
        "model".to_owned(),
        serde_json::Value::String(model.to_owned()),
    );
    write_atomic(&settings_path, &render(root, &settings_path)?)?;
    Ok(true)
}
