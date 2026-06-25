//! Foreign-key-preserving JSON reads and atomic writes for the Claude CLI's
//! registry files. Every write goes through `write_json` so a malformed or
//! foreign file is never silently clobbered.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use super::io_err;
use crate::fsutil;
use crate::sync::ApplyError;

pub fn read_optional_object(path: &Path) -> Result<Option<Map<String, Value>>, ApplyError> {
    let Some(text) =
        fsutil::read_optional(path).map_err(|e| io_err(format!("read {}", path.display()), e))?
    else {
        return Ok(None);
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    if text.trim().is_empty() {
        return Ok(Some(Map::new()));
    }
    // Abort rather than overwrite a file we can't understand — these files hold
    // the user's other plugins and (in settings.json) their auth token.
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => Ok(Some(m)),
        Ok(_) => Err(io_err(
            format!(
                "{} is not a JSON object; refusing to overwrite",
                path.display()
            ),
            std::io::Error::other("unexpected JSON root"),
        )),
        Err(e) => Err(io_err(
            format!("parse {}; refusing to overwrite", path.display()),
            std::io::Error::other(e),
        )),
    }
}

pub(super) fn read_json_object(path: &Path) -> Result<Map<String, Value>, ApplyError> {
    Ok(read_optional_object(path)?.unwrap_or_default())
}

/// An absent or non-object value at `key` is normalised to an empty object, so
/// the `None` arm is unreachable; callers treat `None` as "leave the file
/// untouched" rather than panicking.
pub fn object_entry<'a>(
    root: &'a mut Map<String, Value>,
    key: &'static str,
) -> Option<&'a mut Map<String, Value>> {
    let slot = root.entry(key).or_insert_with(|| Value::Object(Map::new()));
    if !slot.is_object() {
        *slot = Value::Object(Map::new());
    }
    slot.as_object_mut()
}

pub(super) fn write_json(path: &Path, value: &Value) -> Result<(), ApplyError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| io_err(format!("create {}", parent.display()), e))?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| ApplyError::Serialize {
        what: path.display().to_string(),
        source: e,
    })?;
    fsutil::atomic_write_0600(path, &bytes)
        .map_err(|e| io_err(format!("atomic_write {}", path.display()), e))
}
