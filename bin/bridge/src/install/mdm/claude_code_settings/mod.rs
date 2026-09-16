//! Claude Code (terminal CLI) settings on macOS, Linux, and Windows.
//!
//! The `apiKeyHelper` script plus the `env` keys the bridge owns inside the
//! settings file: the machine policy file under
//! [`crate::config::paths::claude_code_policy_dir`] when it is writable
//! (root), otherwise the per-user `~/.claude/settings.json`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod helper;
mod merge;

pub use helper::windows_helper_command;
use helper::{key_helper_path, prepare_helper, shell_command_for};
pub mod model_picker;
pub mod permissions;
mod removal;

use std::fs;
use std::path::{Path, PathBuf};

use crate::fsutil::FileReceipt;
use crate::install::mdm::{MdmApplication, MdmError};

pub(crate) use self::model_picker::apply_model_picker;
pub(crate) use self::removal::{remove_all, remove_managed_settings};

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

// Why: the probe must not leave an empty policy file behind — Claude Code
// reads an empty managed-settings.json as `{}` and the operator sees a
// policy that was never applied. An existing file is opened for write; a
// missing one is judged by whether the nearest existing ancestor accepts a
// temporary file, which the real write path creates and removes.
fn can_write(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => fs::OpenOptions::new().write(true).open(path).is_ok(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => path
            .ancestors()
            .skip(1)
            .find(|dir| dir.is_dir())
            .is_some_and(|dir| tempfile::Builder::new().tempfile_in(dir).is_ok()),
        Ok(_) | Err(_) => false,
    }
}

pub fn standalone_settings_path() -> Option<PathBuf> {
    Some(
        crate::basedirs::config_dir()?
            .join(crate::brand::brand().config_dir)
            .join("claude-code-settings.json"),
    )
}

pub(super) fn bridge_env(gateway: &str) -> serde_json::Map<String, serde_json::Value> {
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

pub(super) fn write_verified(path: &Path, body: &str) -> Result<FileReceipt, MdmError> {
    write_atomic(path, body)?;
    FileReceipt::verify(path, body.as_bytes()).map_err(io_error("verify", path))
}

// Why: the standalone file lets `claude --settings <file>` route one session
// through the gateway without touching `~/.claude/settings.json`, so a
// developer who keeps their own Anthropic login can switch per terminal.
pub(crate) fn write_standalone_settings(gateway: &str) -> Result<MdmApplication, MdmError> {
    let helper = key_helper_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let mut files = prepare_helper(&helper)?;
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
    files.push(standalone_receipt);
    Ok(MdmApplication {
        lines: vec![
            format!(
                "{}: {} (apiKeyHelper)",
                if cfg!(unix) { "wrote" } else { "executable" },
                helper.display()
            ),
            format!(
                "wrote: {} (pass to `claude --settings` to route one session without editing \
                 ~/.claude/settings.json)",
                standalone.display()
            ),
        ],
        files,
        policies: Vec::new(),
    })
}

pub(crate) fn apply_managed_settings(gateway: &str) -> Result<MdmApplication, MdmError> {
    let MdmApplication {
        mut lines,
        mut files,
        ..
    } = write_standalone_settings(gateway)?;
    let helper = key_helper_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    let outcome = (|| {
        let settings_path =
            managed_settings_path().ok_or(MdmError::Resolve("the managed settings path"))?;
        let mut root = read_settings(&settings_path)?;
        merge::merge_bridge_keys(&mut root, &settings_path, gateway, &helper)?;
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
